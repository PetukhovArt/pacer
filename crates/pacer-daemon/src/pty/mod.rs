pub mod capture;
pub mod cloud;
pub mod kill;
pub mod kitty;
pub mod progress;
pub mod ring;

use anyhow::{Context, Result};
use capture::{Capture, CaptureExt};
use cloud::CloudScanner;
use pacer_core::SessionRef;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use progress::ProgressScanner;
use ring::ScrollbackRing;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};

const RING_CAPACITY: usize = 1024 * 1024;
/// Flush coalesced output at this size…
const COALESCE_BYTES: usize = 8 * 1024;
/// …or this long after the first pending byte, whichever comes first. A hard
/// deadline (not a quiet-gap timer): a child streaming continuously in small
/// chunks must still flush on time, or output arrives in laggy 8KB lumps.
const COALESCE_HOLD: std::time::Duration = std::time::Duration::from_millis(5);
/// Reader thread → pump channel bound; blocking_send gives natural
/// backpressure against a fire-hosing child.
const READER_CHANNEL_BOUND: usize = 64;
/// After the polite SIGHUP, how long the child gets to exit before its whole
/// process group is SIGKILLed.
const KILL_GRACE: std::time::Duration = std::time::Duration::from_secs(3);
/// Windows only: how long the child waiter holds its `Eof` after the child
/// exits, so the ConPTY host's final output beats it through the channel
/// (there is no EOF race to lose on Unix — see `spawn_reader_thread`).
#[cfg(windows)]
const EXIT_DRAIN: std::time::Duration = std::time::Duration::from_millis(100);
/// Size a session is spawned at when no client is attached to say better
/// (prewarms, respawns after a move, restarts). The first attach resizes it
/// to the real pane, so these only shape the child's first paint.
pub const DEFAULT_COLS: u16 = 80;
pub const DEFAULT_ROWS: u16 = 24;
/// The two bytes every output scanner in this module keys on: ESC opens a
/// CSI/OSC sequence, BEL is the classic OSC terminator.
pub(crate) const ESC: u8 = 0x1b;
pub(crate) const BEL: u8 = 0x07;

/// A `PtySize` in cells only. Nothing here knows pixel dimensions, and
/// leaving them zero is what every caller wants.
fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Broadcast to attached clients (and, later, the status machine).
#[derive(Clone, Debug)]
pub enum PtyEvent {
    Output {
        seq: u64,
        data: Vec<u8>,
    },
    Exited {
        exit_code: Option<i32>,
    },
    /// The child pushed/popped kitty keyboard flags; clients re-encode keys.
    KittyFlags {
        flags: u8,
    },
    /// win32-input-mode toggled (ConPTY requests it at open; a raw-VT child
    /// may turn it off); clients re-encode keys. Never fires on Unix.
    Win32Input {
        on: bool,
    },
    /// The child's OSC 9;4 progress state flipped. For agent CLIs this is a
    /// busy/idle edge the status machine trusts — notably it is the *only*
    /// end-of-turn signal after the user cancels, which fires no hook.
    Progress {
        busy: bool,
    },
    /// The child printed the id of the Claude Cloud session it created or
    /// attached to. Only scanned for on `--cloud` launches (`arm_cloud_scan`).
    CloudSession {
        id: String,
    },
    /// The child refused to attach to a cloud session ("not enabled for
    /// your account"); it exits right after. Likewise `--cloud` launches only.
    CloudAttachRejected,
}

enum ReaderMsg {
    Data(Vec<u8>),
    Eof { exit_code: Option<i32> },
}

pub struct PtySession {
    pub sref: SessionRef,
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    /// Child pid: drives the SIGHUP → SIGKILL escalation (the child is its
    /// PTY session's leader — portable-pty does setsid — so pgid == pid)
    /// and the metrics modal's process-tree sums.
    pub child_pid: Option<u32>,
    pub ring: Mutex<ScrollbackRing>,
    pub events: broadcast::Sender<PtyEvent>,
    /// Last applied size, for the attach-time SIGWINCH jiggle.
    last_size: Mutex<(u16, u16)>,
    /// Kitty keyboard negotiation state, fed by the pump from live output.
    kitty: Mutex<kitty::KittyScanner>,
    /// ConPTY's `INHERIT_CURSOR` handshake blocks the child until its
    /// `ESC[6n` is answered; the pump answers from here (see `core::dsr`).
    #[cfg(windows)]
    dsr: Mutex<pacer_core::dsr::DsrScanner>,
    /// OSC 9;4 busy/idle tracking, likewise fed from live output.
    progress: Mutex<ProgressScanner>,
    /// Claude Cloud session id / attach-refusal scanner; `None` until a
    /// `--cloud` launch arms it, so ordinary sessions pay nothing.
    cloud: Mutex<Option<CloudScanner>>,
    /// The child's process tree, claimed at spawn: what the kill watchdog
    /// reaches for when the child outlives its hangup (see `pty::kill`).
    group: kill::ProcessGroup,
    /// Raw-output capture, off unless `PACER_PTY_CAPTURE` is set.
    capture: Mutex<Option<Capture>>,
    /// Set by the first `write_input`. A Cloud mirror stops re-teleporting
    /// once its pane has been typed into: the moment the user talks to the
    /// local session, replacing it under them would eat their turn.
    /// Resizes and attaches deliberately do not count.
    input_seen: AtomicBool,
}

pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: std::path::PathBuf,
    /// Extra env vars (PACER_* for agents). Plain terminals get none.
    pub env: Vec<(String, String)>,
    /// Env var names to scrub from the inherited environment. Only ever a
    /// fixed list (the agent-session vars), so it is borrowed, not built.
    pub scrub_env: &'static [&'static str],
    pub cols: u16,
    pub rows: u16,
}

impl PtySession {
    /// Spawn the child in a fresh PTY and start its reader thread + pump task.
    pub fn spawn(sref: SessionRef, spec: SpawnSpec) -> Result<Arc<Self>> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(pty_size(spec.cols, spec.rows))
            .context("openpty")?;

        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        cmd.env("TERM", "xterm-256color");
        for name in spec.scrub_env {
            cmd.env_remove(name);
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("spawn child in pty")?;
        drop(pair.slave);

        let killer = child.clone_killer();
        let child_pid = child.process_id();
        // Claimed here, at the earliest moment the child exists, because the
        // group is what reaches its grandchildren later (see `pty::kill`).
        let group = kill::ProcessGroup::claim(child_pid);
        let reader = pair.master.try_clone_reader().context("clone pty reader")?;
        let writer = pair.master.take_writer().context("take pty writer")?;

        let (events, _) = broadcast::channel(256);
        let mut capture = Capture::open(&sref);
        capture.resize(spec.cols, spec.rows);
        let session = Arc::new(Self {
            sref,
            writer: Mutex::new(writer),
            master: Mutex::new(pair.master),
            killer: Mutex::new(killer),
            child_pid,
            group,
            ring: Mutex::new(ScrollbackRing::new(RING_CAPACITY)),
            events,
            last_size: Mutex::new((spec.cols, spec.rows)),
            kitty: Mutex::new(kitty::KittyScanner::new()),
            #[cfg(windows)]
            dsr: Mutex::new(pacer_core::dsr::DsrScanner::new()),
            progress: Mutex::new(ProgressScanner::new()),
            cloud: Mutex::new(None),
            capture: Mutex::new(capture),
            input_seen: AtomicBool::new(false),
        });

        let (tx, rx) = mpsc::channel::<ReaderMsg>(READER_CHANNEL_BOUND);
        spawn_reader_thread(reader, child, tx);
        tokio::spawn(pump(session.clone(), rx));
        Ok(session)
    }

    pub fn write_input(&self, data: &[u8]) -> Result<()> {
        if !data.is_empty() {
            self.input_seen.store(true, Ordering::Relaxed);
        }
        self.write_reply(data)
    }

    /// Write bytes the *daemon* owes the child (kitty/DA1/DSR replies) —
    /// same pipe as `write_input`, but not user input, so it must not trip
    /// `input_seen` (the Cloud mirror's "typed into" rule).
    fn write_reply(&self, data: &[u8]) -> Result<()> {
        let mut w = self.writer.lock().unwrap();
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let master = self.master.lock().unwrap();
        master.resize(pty_size(cols, rows))?;
        *self.last_size.lock().unwrap() = (cols, rows);
        self.capture.lock().unwrap().resize(cols, rows);
        Ok(())
    }

    /// Resize on attach. The kernel only delivers SIGWINCH on a *change*, so
    /// when the requested size equals the current one, jiggle (rows-1 then
    /// back) to force a full-screen repaint — the dtach trick.
    pub fn resize_with_jiggle(&self, cols: u16, rows: u16) -> Result<()> {
        let same = { *self.last_size.lock().unwrap() == (cols, rows) };
        if same && rows > 1 {
            let master = self.master.lock().unwrap();
            master.resize(pty_size(cols, rows - 1))?;
            master.resize(pty_size(cols, rows))?;
            Ok(())
        } else {
            self.resize(cols, rows)
        }
    }

    /// Hang the child up politely, then kill its whole process group if it
    /// hasn't exited within [`KILL_GRACE`]. The group kill also reaps
    /// grandchildren that would otherwise hold the slave open (no EOF →
    /// reader thread, pump task, and the 1MB ring all pinned forever).
    pub fn kill(&self) {
        // Subscribe before signalling so an immediate exit can't be missed.
        let mut rx = self.events.subscribe();
        let _ = self.killer.lock().unwrap().kill();
        let Some(pid) = self.child_pid else { return };
        let sref = self.sref.clone();
        // The watchdog carries its own clone of the group, so the group
        // outlives this session being dropped mid-grace.
        let group = self.group.clone();
        // Watchdog on a plain thread: it must not hold the session Arc (that
        // would pin the ring), and it outlives any tokio context `kill` was
        // called from.
        std::thread::Builder::new()
            .name("pty-kill-watchdog".into())
            .stack_size(64 * 1024)
            .spawn(move || {
                let deadline = std::time::Instant::now() + KILL_GRACE;
                while std::time::Instant::now() < deadline {
                    loop {
                        use tokio::sync::broadcast::error::TryRecvError;
                        match rx.try_recv() {
                            Ok(PtyEvent::Exited { .. }) | Err(TryRecvError::Closed) => return,
                            Ok(_) | Err(TryRecvError::Lagged(_)) => continue,
                            Err(TryRecvError::Empty) => break,
                        }
                    }
                    // The leader being gone strictly precedes the Exited
                    // broadcast, so this also covers an Exited lost to lag.
                    if !group.leader_alive() {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                tracing::warn!(session = ?sref, pid, "child ignored the hangup — killing its process group");
                group.kill_all();
            })
            .expect("spawn pty kill watchdog");
    }

    /// Ring snapshot for attach replay: (base_seq, bytes).
    pub fn snapshot(&self, from_seq: Option<u64>) -> (u64, Vec<u8>) {
        self.ring.lock().unwrap().snapshot_from(from_seq)
    }

    /// The child's current kitty keyboard flags (0 = legacy).
    pub fn kitty_flags(&self) -> u8 {
        self.kitty.lock().unwrap().flags()
    }

    /// Whether the session's terminal side is in win32-input-mode.
    pub fn win32_input(&self) -> bool {
        self.kitty.lock().unwrap().win32_input()
    }

    /// The child's advertised OSC 9;4 busy state, or `None` if it never
    /// advertised one (a CLI without a progress bar, or one not started yet).
    pub fn progress_busy(&self) -> Option<bool> {
        self.progress.lock().unwrap().busy()
    }

    /// Start watching this child's output for the Claude Cloud session id
    /// it prints on creation and for an attach refusal (see `pty::cloud`).
    /// Output that already landed in the ring is scanned first, so arming
    /// a moment after spawn cannot miss a fast-printing child; sightings
    /// then arrive as `PtyEvent::CloudSession` / `CloudAttachRejected`.
    /// Whether anything has been typed into this session. Drives the
    /// Cloud mirror's "stop refreshing once it's yours" rule.
    pub fn input_seen(&self) -> bool {
        self.input_seen.load(Ordering::Relaxed)
    }

    pub fn arm_cloud_scan(&self) {
        let mut scanner = CloudScanner::new();
        let (_, replay) = self.snapshot(None);
        let sightings = scanner.feed(&replay);
        *self.cloud.lock().unwrap() = Some(scanner);
        for sighting in sightings {
            let _ = self.events.send(sighting.into());
        }
    }
}

/// PTY reads are blocking → dedicated thread per session.
///
/// Who notices the exit differs by platform. On Unix the child's death
/// closes the slave, the read returns EOF, and the same thread reaps the
/// child for its exit code. On Windows the ConPTY host holds the master
/// pipe open until `ClosePseudoConsole`, so EOF never announces the exit —
/// a separate waiter thread reaps the child and sends `Eof` itself, after
/// [`EXIT_DRAIN`] so the host's final bytes beat it through the channel.
fn spawn_reader_thread(
    mut reader: Box<dyn Read + Send>,
    #[allow(unused_mut)] mut child: Box<dyn portable_pty::Child + Send + Sync>,
    tx: mpsc::Sender<ReaderMsg>,
) {
    #[cfg(windows)]
    let tx_reader = {
        let mut child = child;
        let tx_waiter = tx.clone();
        std::thread::Builder::new()
            .name("pty-child-waiter".into())
            .stack_size(64 * 1024)
            .spawn(move || {
                let exit_code = child.wait().ok().map(|st| st.exit_code() as i32);
                std::thread::sleep(EXIT_DRAIN);
                let _ = tx_waiter.blocking_send(ReaderMsg::Eof { exit_code });
            })
            .expect("spawn pty child waiter thread");
        tx
    };
    #[cfg(unix)]
    let tx_reader = tx;

    std::thread::Builder::new()
        .name("pty-reader".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let mut buf = [0u8; 16 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx_reader
                            .blocking_send(ReaderMsg::Data(buf[..n].to_vec()))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            // Unix: EOF is how the exit announces itself — reap here.
            // Windows: the waiter owns the child and already reported it.
            #[cfg(unix)]
            {
                let exit_code = child.wait().ok().map(|st| st.exit_code() as i32);
                let _ = tx_reader.blocking_send(ReaderMsg::Eof { exit_code });
            }
        })
        .expect("spawn pty reader thread");
}

/// Drains the reader channel: append to the ring (always — detach is free),
/// coalesce bursts, broadcast to whoever is attached.
async fn pump(session: Arc<PtySession>, mut rx: mpsc::Receiver<ReaderMsg>) {
    let mut pending: Vec<u8> = Vec::new();

    let flush = |session: &PtySession, pending: &mut Vec<u8>| {
        if pending.is_empty() {
            return;
        }
        // The ConPTY host's cursor query gates the child's launch, so it is
        // answered first, before anything else looks at the bytes.
        #[cfg(windows)]
        {
            let hits = session.dsr.lock().unwrap().feed(pending);
            for _ in 0..hits {
                if let Err(e) = session.write_reply(pacer_core::dsr::DSR_REPLY) {
                    tracing::warn!(error = %e, "DSR reply write failed");
                }
            }
        }
        // Kitty keyboard negotiation rides in the output stream; nothing else
        // would ever answer the child's queries (tmux does the same).
        let actions = session.kitty.lock().unwrap().feed(pending);
        if !actions.reply.is_empty() {
            if let Err(e) = session.write_reply(&actions.reply) {
                tracing::warn!(error = %e, "kitty/DA reply write failed");
            }
        }
        let busy_edge = session.progress.lock().unwrap().feed(pending);
        let cloud_sightings = match session.cloud.lock().unwrap().as_mut() {
            Some(scanner) => scanner.feed(pending),
            None => Vec::new(),
        };
        session.capture.lock().unwrap().output(pending);
        let seq = session.ring.lock().unwrap().append(pending);
        let _ = session.events.send(PtyEvent::Output {
            seq,
            data: std::mem::take(pending),
        });
        if let Some(flags) = actions.flags_changed {
            tracing::debug!(session = ?session.sref, flags, "child kitty flags changed");
            let _ = session.events.send(PtyEvent::KittyFlags { flags });
        }
        if let Some(on) = actions.win32_changed {
            tracing::debug!(session = ?session.sref, on, "win32-input-mode toggled");
            let _ = session.events.send(PtyEvent::Win32Input { on });
        }
        if let Some(busy) = busy_edge {
            tracing::debug!(session = ?session.sref, busy, "child progress state changed");
            let _ = session.events.send(PtyEvent::Progress { busy });
        }
        for sighting in cloud_sightings {
            tracing::info!(session = ?session.sref, ?sighting, "cloud sighting in child output");
            let _ = session.events.send(sighting.into());
        }
    };

    'outer: loop {
        if pending.is_empty() {
            match rx.recv().await {
                Some(ReaderMsg::Data(d)) => pending.extend_from_slice(&d),
                Some(ReaderMsg::Eof { exit_code }) => {
                    let _ = session.events.send(PtyEvent::Exited { exit_code });
                    break;
                }
                None => break,
            }
        }
        // Coalesce until the deadline or the size cap; the deadline is fixed
        // at the first pending byte so continuous streams still flush on time.
        let deadline = tokio::time::Instant::now() + COALESCE_HOLD;
        while pending.len() < COALESCE_BYTES {
            tokio::select! {
                msg = rx.recv() => match msg {
                    Some(ReaderMsg::Data(d)) => pending.extend_from_slice(&d),
                    Some(ReaderMsg::Eof { exit_code }) => {
                        flush(&session, &mut pending);
                        let _ = session.events.send(PtyEvent::Exited { exit_code });
                        break 'outer;
                    }
                    None => {
                        flush(&session, &mut pending);
                        break 'outer;
                    }
                },
                _ = tokio::time::sleep_until(deadline) => break,
            }
        }
        flush(&session, &mut pending);
    }
    tracing::info!(session = ?session.sref, "pty pump ended");
}

#[cfg(test)]
mod tests {
    use super::*;
    use pacer_core::AgentId;

    /// Any long-lived child will do — the assertions are about what the
    /// session records, not what the child does with it. It just has to be a
    /// program that exists: `/bin/sh` is an MSYS mapping, not a path
    /// `CreateProcess` can resolve, so Windows idles in PowerShell.
    fn idle_child() -> (String, Vec<String>) {
        #[cfg(unix)]
        {
            ("/bin/cat".into(), vec![])
        }
        #[cfg(windows)]
        {
            (
                "powershell.exe".into(),
                ["-NoProfile", "-Command", "Start-Sleep 30"]
                    .map(String::from)
                    .to_vec(),
            )
        }
    }

    fn echo_session() -> Arc<PtySession> {
        let (program, args) = idle_child();
        PtySession::spawn(
            SessionRef::Agent(AgentId::generate()),
            SpawnSpec {
                program,
                args,
                cwd: std::env::temp_dir(),
                env: vec![],
                scrub_env: &[],
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
            },
        )
        .unwrap()
    }

    /// A child that prints `marker` and exits — for asserting on both ends
    /// of a session's life.
    fn print_child(marker: &str) -> (String, Vec<String>) {
        #[cfg(unix)]
        {
            (
                "/bin/sh".into(),
                vec!["-c".into(), format!("printf '{marker}'")],
            )
        }
        #[cfg(windows)]
        {
            (
                "powershell.exe".into(),
                vec![
                    "-NoProfile".into(),
                    "-Command".into(),
                    format!("Write-Host {marker}"),
                ],
            )
        }
    }

    /// The whole life of a PTY SESSION: the child runs, its output lands in
    /// the SCROLLBACK RING, and its exit is broadcast. On Windows each step
    /// has its own machinery to regress — the pump's DSR reply is what lets
    /// the ConPTY child start at all, and the waiter thread is what turns
    /// its exit into `Exited` (the master pipe never EOFs on it).
    #[tokio::test]
    async fn child_output_reaches_the_ring_and_the_exit_is_reported() {
        let (program, args) = print_child("PTY_RING_MARKER");
        let session = PtySession::spawn(
            SessionRef::Agent(AgentId::generate()),
            SpawnSpec {
                program,
                args,
                cwd: std::env::temp_dir(),
                env: vec![],
                scrub_env: &[],
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
            },
        )
        .unwrap();
        let mut rx = session.events.subscribe();
        let exited = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                match rx.recv().await {
                    Ok(PtyEvent::Exited { .. }) => break,
                    Ok(_) => continue,
                    Err(e) => panic!("event stream ended without Exited: {e}"),
                }
            }
        })
        .await;
        let (_, replay) = session.snapshot(None);
        let text = String::from_utf8_lossy(&replay);
        assert!(
            exited.is_ok(),
            "no Exited within 10s; ring so far: {text:?}"
        );
        assert!(
            text.contains("PTY_RING_MARKER"),
            "child output never reached the ring: {text:?}"
        );
    }

    /// ConPTY opens every session with `CSI ? 9001 h`; the pump must turn
    /// that into a `Win32Input { on: true }` broadcast, or no client ever
    /// switches to the encoding that carries Shift+Enter to cooked children.
    #[cfg(windows)]
    #[tokio::test]
    async fn conpty_win32_input_mode_is_broadcast() {
        let session = echo_session();
        let mut rx = session.events.subscribe();
        let seen = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                match rx.recv().await {
                    Ok(PtyEvent::Win32Input { on: true }) => break,
                    Ok(_) => continue,
                    Err(e) => panic!("event stream ended first: {e}"),
                }
            }
        })
        .await;
        assert!(
            seen.is_ok(),
            "no Win32Input within 10s — is ConPTY no longer requesting ?9001h, \
             or did the scanner stop seeing it?"
        );
        assert!(session.win32_input(), "the attach-time accessor agrees");
        session.kill();
    }

    /// The Cloud mirror stops refreshing a pane once its user has typed
    /// into it, so `input_seen` must track keystrokes only — an attach's
    /// resize jiggle happens without anyone touching the keyboard.
    /// Opens a real PTY; on Windows the pump's DSR reply is what lets the
    /// ConPTY child start (and thus be reaped) at all.
    #[tokio::test]
    async fn input_seen_tracks_keystrokes_not_resizes() {
        let session = echo_session();
        assert!(!session.input_seen());
        session.resize_with_jiggle(100, 30).unwrap();
        session.resize(80, 24).unwrap();
        assert!(!session.input_seen(), "a resize is not input");
        session.write_input(b"").unwrap();
        assert!(!session.input_seen(), "an empty write is not input");
        session.write_input(b"x").unwrap();
        assert!(session.input_seen());
        session.kill();
    }
}
