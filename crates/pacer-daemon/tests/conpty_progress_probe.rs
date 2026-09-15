//! Where pacer's end-of-turn signal comes from on Windows, pinned against
//! the real ConPTY and the real Claude Code.
//!
//! A cancelled turn fires no `Stop` hook and suppresses the idle
//! notification, so the only way back out of `running` is something read off
//! the child's own output. `pty::progress` reads OSC 9;4 for that;
//! `pty::title` reads the window-title glyph because Claude Code 2.1.270
//! stopped emitting OSC 9;4 altogether.
//!
//! One test runs on the grid: ConPTY passes OSC 9;4 through, so if the
//! progress scanner ever goes blind it is the CLI's doing and not the
//! platform's. The two `#[ignore]`d probes drive the real `claude` on PATH
//! (they spend tokens and take a minute) and exist for hand-run diagnosis —
//! run them when an agent wedges on yellow, or to re-check a Claude Code
//! release against the glyph `pty::title` keys on.

#![cfg(windows)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// Spawn a command under a fresh ConPTY, answer the `INHERIT_CURSOR`
/// handshake, and call `drive` with everything read so far after each chunk
/// so the caller can inject input at the right moment. Returns the whole
/// master-side transcript.
fn drive_pty(
    program: &str,
    args: &[&str],
    window_secs: u64,
    mut drive: impl FnMut(&str, &mut dyn Write),
) -> Vec<u8> {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut cmd = CommandBuilder::new(program);
    for a in args {
        cmd.arg(a);
    }
    cmd.cwd(std::env::current_dir().unwrap());
    // A nested Claude Code inherits markers that change how it behaves.
    for (k, _) in std::env::vars() {
        if k.starts_with("CLAUDE") || k.starts_with("PACER") {
            cmd.env_remove(&k);
        }
    }
    let mut child = pty.slave.spawn_command(cmd).unwrap();
    drop(pty.slave);
    let mut reader = pty.master.try_clone_reader().unwrap();
    let mut writer = pty.master.take_writer().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut out = Vec::new();
    let mut answered_dsr = false;
    let deadline = Instant::now() + Duration::from_secs(window_secs);
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(chunk) => out.extend_from_slice(&chunk),
            Err(_) => {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
            }
        }
        // Whoever reads the master must answer the host's ESC[6n or the
        // child never runs at all.
        if !answered_dsr && out.windows(4).any(|w| w == b"\x1b[6n") {
            writer.write_all(b"\x1b[1;1R").unwrap();
            writer.flush().unwrap();
            answered_dsr = true;
        }
        let text = String::from_utf8_lossy(&out).to_string();
        drive(&text, &mut writer);
    }
    let _ = child.kill();
    out
}

/// Every OSC 9;4 state the child advertised, in order.
fn progress_states(text: &str) -> Vec<char> {
    let mut v = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("\x1b]9;4;") {
        rest = &rest[i + 6..];
        if let Some(c) = rest.chars().next() {
            v.push(c);
        }
    }
    v
}

/// The leading glyph of every OSC 0 title the child set, in order, with
/// consecutive repeats collapsed — that is the busy/idle edge `pty::title`
/// keys on.
fn title_glyphs(text: &str) -> Vec<char> {
    let mut v: Vec<char> = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("\x1b]0;") {
        rest = &rest[i + 4..];
        let Some(glyph) = rest.chars().next() else {
            continue;
        };
        if v.last() != Some(&glyph) {
            v.push(glyph);
        }
    }
    v
}

/// Drive a real `claude`: type a prompt that takes a while, submit it, and
/// cancel with escape once it is under way. The cancel is on a timer rather
/// than on a busy edge because whether any busy edge exists is the thing
/// under test.
fn claude_turn_then_cancel(window_secs: u64, cancel_after: Option<Duration>) -> String {
    let prompt = "write 300 words about the number seven";
    let mut typed_at: Option<Instant> = None;
    let mut submitted_at: Option<Instant> = None;
    let mut cancelled = false;
    let out = drive_pty(
        "claude.cmd",
        &["--permission-mode", "plan"],
        window_secs,
        |text, w| {
            if typed_at.is_none() {
                // The input box's placeholder hint: the CLI is up and
                // listening. Its own text is written with column jumps
                // between the words, so it is never matchable whole.
                if text.contains("Try") {
                    w.write_all(prompt.as_bytes()).unwrap();
                    w.flush().unwrap();
                    typed_at = Some(Instant::now());
                }
                return;
            }
            if submitted_at.is_none() {
                if typed_at.unwrap().elapsed() > Duration::from_secs(2) {
                    w.write_all(b"\r").unwrap();
                    w.flush().unwrap();
                    submitted_at = Some(Instant::now());
                }
                return;
            }
            if let (Some(after), false) = (cancel_after, cancelled) {
                if submitted_at.unwrap().elapsed() > after {
                    w.write_all(b"\x1b").unwrap();
                    w.flush().unwrap();
                    cancelled = true;
                }
            }
        },
    );
    assert!(submitted_at.is_some(), "the probe never submitted a prompt");
    assert!(
        cancel_after.is_none() || cancelled,
        "the probe never got to press escape"
    );
    String::from_utf8_lossy(&out).into_owned()
}

/// The platform assumption under `pty::progress`: a child's OSC 9;4 reaches
/// the master reader. It does — so a progress scanner that reads nothing is
/// a CLI that emits nothing, never ConPTY eating it.
#[test]
fn osc_9_4_in_child_output_survives_conpty() {
    let out = drive_pty(
        "powershell.exe",
        &[
            "-NoProfile",
            "-Command",
            "[Console]::Write([char]27+']9;4;3;'+[char]7); Start-Sleep -Milliseconds 300; \
             [Console]::Write([char]27+']9;4;0;'+[char]7); [Console]::Write('MARK-DONE')",
        ],
        15,
        |_, _| {},
    );
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("MARK-DONE"), "the child never ran: {text:?}");
    assert_eq!(
        progress_states(&text),
        vec!['3', '0'],
        "ConPTY ate the progress edges: {text:?}"
    );
}

/// Hand-run diagnosis: what a real Claude Code turn advertises, and what
/// survives an escape cancel.
///
/// Observed 2026-09-15 against Claude Code 2.1.270 under the Windows 10
/// inbox ConPTY: **no OSC 9;4 at all** — not at startup, not on submit, not
/// on the cancel — and the title glyph going `✳` → `◐`/`◑` → `✳`, the last
/// flip arriving on the cancel. That is the whole reason `pty::title`
/// exists. A release that brings the progress bar back would show `0`,`3`,`0`
/// here and make the title redundant again.
#[test]
#[ignore]
fn probe_claude_cancel_signal() {
    let text = claude_turn_then_cancel(60, Some(Duration::from_secs(8)));
    eprintln!("[probe] progress states: {:?}", progress_states(&text));
    eprintln!("[probe] title glyphs: {:?}", title_glyphs(&text));
    assert_eq!(
        title_glyphs(&text).last(),
        Some(&'✳'),
        "the title did not go idle on the cancel — `pty::title::IDLE_GLYPH` \
         is stale, and cancelled turns now wedge on yellow"
    );
}

/// Hand-run diagnosis: the reason the title may only close out a turn that
/// is already `running`.
///
/// Observed 2026-09-15 against Claude Code 2.1.270: a plan-approval prompt
/// sits at the **idle** glyph for as long as it waits. Reading the title as
/// a plain end-of-turn signal would therefore green out every agent that is
/// genuinely waiting on the user — see the gate in `status::AgentStatusMachine`.
#[test]
#[ignore]
fn probe_claude_title_while_a_prompt_waits() {
    let prompt = "make a one sentence plan to add a comment at the top of README.md, \
                  then present it for approval";
    let mut typed_at: Option<Instant> = None;
    let mut submitted = false;
    let out = drive_pty(
        "claude.cmd",
        &["--permission-mode", "plan"],
        120,
        |text, w| {
            if typed_at.is_none() {
                if text.contains("Try") {
                    w.write_all(prompt.as_bytes()).unwrap();
                    w.flush().unwrap();
                    typed_at = Some(Instant::now());
                }
                return;
            }
            if !submitted && typed_at.unwrap().elapsed() > Duration::from_secs(2) {
                w.write_all(b"\r").unwrap();
                w.flush().unwrap();
                submitted = true;
            }
        },
    );
    let text = String::from_utf8_lossy(&out);
    eprintln!("[probe] title glyphs: {:?}", title_glyphs(&text));
    eprintln!("[probe] progress states: {:?}", progress_states(&text));
    assert!(
        text.contains("proceed") || text.contains("Yes, and"),
        "no approval prompt was reached, so the probe proves nothing"
    );
    assert_eq!(
        title_glyphs(&text).last(),
        Some(&'✳'),
        "the title no longer reads idle while a prompt waits — the \
         `needs_feedback` gate on TitleIdle could be relaxed"
    );
}
