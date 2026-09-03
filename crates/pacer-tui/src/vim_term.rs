//! Embedded editor modal: a local PTY child (vim) rendered inside the TUI.
//!
//! Unlike agent/terminal sessions (daemon-owned PTYs reached over IPC), the
//! editor is spawned in-process: it's a short-lived affordance of the
//! find-in-files overlay, needs no persistence or reattach, and dies with
//! the client. Output flows reader thread → mpsc → the main loop, which
//! feeds the vt100 parser here (the daemon's `PtySession` shape, minus the
//! ring buffer and broadcast).

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use ratatui::layout::Rect;
use std::io::{Read, Write};
use std::path::Path;
use tokio::sync::mpsc::UnboundedSender;

/// Reader-thread → main-loop messages. `generation` stamps which spawn they
/// belong to, so bytes from a closed editor can't bleed into a new one.
#[derive(Debug)]
pub enum VimEvent {
    Output { generation: u64, data: Vec<u8> },
    Exited { generation: u64 },
}

pub struct VimTerm {
    pub generation: u64,
    pub parser: vt100::Parser,
    /// Size the parser (and PTY) currently uses.
    pub cols: u16,
    pub rows: u16,
    /// "path:line" for the modal title.
    pub title: String,
    /// Rendered inside the tree browser's preview pane instead of the
    /// centered modal (set by the tree browser's Enter).
    pub embedded: bool,
    /// Inner rect from the last draw; `sync_vim_size` resizes to it.
    pub area: Rect,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    /// ConPTY's `INHERIT_CURSOR` handshake blocks the child until its
    /// `ESC[6n` is answered; `process` answers (see `pacer_core::dsr`).
    #[cfg(windows)]
    dsr: pacer_core::dsr::DsrScanner,
}

/// The `LANG` a Windows editor child needs, or `None` when the environment
/// already names a locale.
///
/// Git for Windows ships MSYS builds of vim and nano, which take their
/// charset from the locale. A PowerShell (or Explorer-launched) environment
/// states none, and vim then starts at `encoding=latin1` with
/// `fileencodings=ucs-bom`: every UTF-8 file opens as mojibake, non-ASCII
/// bytes painted as `~B` control notation. Only a vacuum is filled — an
/// `LC_ALL`/`LC_CTYPE`/`LANG` the user set is their answer, non-UTF-8
/// included.
#[cfg(windows)]
fn utf8_locale_fallback(lookup: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .all(|var| lookup(var).is_none())
        .then_some("C.UTF-8")
}

impl VimTerm {
    /// Spawn `editor +<line> <file>` in the checkout. `Err` is a user-facing
    /// flash message.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_editor(
        editor: &str,
        root: &Path,
        file: &str,
        line: u64,
        cols: u16,
        rows: u16,
        generation: u64,
        tx: UnboundedSender<VimEvent>,
    ) -> Result<Self, String> {
        let title = format!("{file}:{line}");
        Self::spawn_cmd(
            editor,
            &[format!("+{line}"), file.to_string()],
            root,
            title,
            cols,
            rows,
            generation,
            tx,
        )
    }

    /// Editor-agnostic spawn (tests use a shell here).
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_cmd(
        program: &str,
        args: &[String],
        cwd: &Path,
        title: String,
        cols: u16,
        rows: u16,
        generation: u64,
        tx: UnboundedSender<VimEvent>,
    ) -> Result<Self, String> {
        let cols = cols.max(2);
        let rows = rows.max(2);
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("openpty failed: {e}"))?;

        // CreateProcess does no PATH × PATHEXT search, and the default
        // "vim" is rarely on a Windows PATH anyway — resolve it (with the
        // Git-for-Windows fallback) before spawning. Unresolved names go
        // through untouched so the OS's own error surfaces.
        #[cfg(windows)]
        let resolved =
            pacer_core::spawn::resolve_editor_program(program).map(|p| p.display().to_string());
        #[cfg(windows)]
        let launch: &str = resolved.as_deref().unwrap_or(program);
        #[cfg(unix)]
        let launch: &str = program;

        let mut cmd = CommandBuilder::new(launch);
        cmd.args(args);
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        // Git for Windows ships MSYS builds of vim and nano, which take
        // their charset from the locale — see `utf8_locale_fallback`.
        #[cfg(windows)]
        if let Some(lang) = utf8_locale_fallback(pacer_core::env::non_empty) {
            cmd.env("LANG", lang);
        }

        let mut child = pair.slave.spawn_command(cmd).map_err(|e| {
            let msg = format!("failed to launch {program}: {e}");
            #[cfg(windows)]
            let msg = if resolved.is_none() {
                format!("{msg} (not on PATH — set PACER_EDITOR or pick another editor in Settings)")
            } else {
                msg
            };
            msg
        })?;
        drop(pair.slave);

        let killer = child.clone_killer();
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("pty reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("pty writer: {e}"))?;

        // On Windows the ConPTY host holds the master pipe open after the
        // child exits (EOF arrives only at ClosePseudoConsole), so a blocked
        // read can't be what notices the exit: a separate waiter reaps the
        // child and sends `Exited`, after a short drain window so the host's
        // final bytes beat it through the channel.
        #[cfg(windows)]
        let tx_reader = {
            let tx_waiter = tx.clone();
            std::thread::spawn(move || {
                let _ = child.wait(); // reap
                std::thread::sleep(std::time::Duration::from_millis(100));
                let _ = tx_waiter.send(VimEvent::Exited { generation });
            });
            tx
        };
        #[cfg(unix)]
        let tx_reader = tx;

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if tx_reader
                            .send(VimEvent::Output { generation, data })
                            .is_err()
                        {
                            break; // main loop gone
                        }
                    }
                }
            }
            // Unix: EOF is how the exit announces itself — reap here.
            // Windows: the waiter owns the child and already reported it.
            #[cfg(unix)]
            {
                let _ = child.wait(); // reap
                let _ = tx_reader.send(VimEvent::Exited { generation });
            }
        });

        Ok(Self {
            generation,
            parser: vt100::Parser::new(rows, cols, 0),
            cols,
            rows,
            title,
            embedded: false,
            area: Rect::default(),
            master: pair.master,
            writer,
            killer,
            #[cfg(windows)]
            dsr: pacer_core::dsr::DsrScanner::new(),
        })
    }

    /// Feed reader-thread output into the emulator.
    pub fn process(&mut self, data: &[u8]) {
        #[cfg(windows)]
        {
            let hits = self.dsr.feed(data);
            for _ in 0..hits {
                let _ = self
                    .writer
                    .write_all(pacer_core::dsr::DSR_REPLY)
                    .and_then(|_| self.writer.flush());
            }
        }
        self.parser.process(data);
    }

    /// Encoded keystrokes (and bracketed pastes) go straight to the child.
    pub fn input(&mut self, data: &[u8]) {
        // A write error means the child died; the Exited event closes us.
        let _ = self
            .writer
            .write_all(data)
            .and_then(|_| self.writer.flush());
    }

    /// Keep the PTY and parser sized to the drawn modal.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let (cols, rows) = (cols.max(2), rows.max(2));
        if (self.cols, self.rows) == (cols, rows) {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        self.parser.screen_mut().set_size(rows, cols);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    /// Force-close (the Ctrl+Q hatch); the reader thread reaps the child.
    pub fn kill(&mut self) {
        let _ = self.killer.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn recv_until(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<VimEvent>,
        term: &mut VimTerm,
        mut done: impl FnMut(&VimTerm, &VimEvent) -> bool,
    ) {
        let ok = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(ev) = rx.recv().await {
                if let VimEvent::Output { data, .. } = &ev {
                    term.process(data);
                }
                if done(term, &ev) {
                    return;
                }
            }
            panic!("event channel closed early");
        })
        .await;
        assert!(
            ok.is_ok(),
            "timed out; screen:\n{}",
            term.parser.screen().contents()
        );
    }

    // On Windows this test only passes because `process` answers the ConPTY
    // host's `ESC[6n` — the child's launch is gated on that reply
    // (see `pacer_core::dsr`).
    #[tokio::test]
    async fn output_reaches_parser_and_kill_exits() {
        let dir = tempfile::tempdir().unwrap();
        let stub = crate::editor_stub::prints_then_idles("VIM_MODAL_TEST");
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut term =
            VimTerm::spawn_cmd(&stub.0, &stub.1, dir.path(), "test".into(), 80, 24, 1, tx).unwrap();

        recv_until(&mut rx, &mut term, |t, _| {
            t.parser.screen().contents().contains("VIM_MODAL_TEST")
        })
        .await;

        term.kill();
        recv_until(&mut rx, &mut term, |_, ev| {
            matches!(ev, VimEvent::Exited { generation: 1 })
        })
        .await;
    }

    #[tokio::test]
    async fn input_reaches_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let stub = crate::editor_stub::echoes_one_line();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut term =
            VimTerm::spawn_cmd(&stub.0, &stub.1, dir.path(), "test".into(), 80, 24, 7, tx).unwrap();

        term.input(b"hello\r");
        recv_until(&mut rx, &mut term, |t, _| {
            t.parser.screen().contents().contains("GOT:hello")
        })
        .await;
        // The script ends after one line: the exit must be reported with the
        // spawn's generation stamp.
        recv_until(&mut rx, &mut term, |_, ev| {
            matches!(ev, VimEvent::Exited { generation: 7 })
        })
        .await;
    }

    #[tokio::test]
    async fn spawn_failure_is_a_message_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let err = VimTerm::spawn_cmd(
            "/nonexistent-editor-binary",
            &[],
            dir.path(),
            "test".into(),
            80,
            24,
            1,
            tx,
        )
        .map(|_| ())
        .unwrap_err();
        assert!(err.contains("failed to launch"), "{err}");
    }

    #[cfg(windows)]
    #[test]
    fn only_an_unstated_locale_gets_the_utf8_fallback() {
        assert_eq!(utf8_locale_fallback(|_| None), Some("C.UTF-8"));
        for stated in ["LC_ALL", "LC_CTYPE", "LANG"] {
            let lookup = |var: &str| (var == stated).then(|| "ru_RU.CP1251".to_string());
            assert_eq!(
                utf8_locale_fallback(lookup),
                None,
                "{stated} is the user's answer"
            );
        }
    }

    /// The bug: with no locale in the environment, Git for Windows' vim
    /// opens a UTF-8 file as latin1 and paints mojibake.
    #[cfg(windows)]
    #[tokio::test]
    async fn a_utf8_file_opens_as_utf8() {
        if utf8_locale_fallback(pacer_core::env::non_empty).is_none() {
            return; // A stated locale owns the answer; nothing of ours runs.
        }
        let Some(_vim) = pacer_core::spawn::resolve_editor_program("vim") else {
            return; // No vim on this box.
        };
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("utf8.md"),
            "# Что сделано
",
        )
        .unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // -u NONE: a developer vimrc setting `encoding` would mask the bug.
        let args = ["-u", "NONE", "-N", "utf8.md"].map(String::from);
        let mut term =
            VimTerm::spawn_cmd("vim", &args, dir.path(), "test".into(), 80, 24, 1, tx).unwrap();

        recv_until(&mut rx, &mut term, |t, _| {
            t.parser.screen().contents().contains("Что сделано")
        })
        .await;

        term.kill();
    }
}
