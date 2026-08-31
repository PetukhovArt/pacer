//! Hand-run probe: what Claude Code, under ConPTY, does with the kitty
//! newline chords the TUI encodes — `CSI 13;2u` (Shift+Enter) and
//! `CSI 13;3u` (Alt+Enter). Both insert a newline; observed 2026-08 against
//! Claude Code v2.1.251, which negotiates kitty flags 5 under ConPTY.
//!
//! Needs `claude` on PATH and starts a real (unsubmitted) session, so it is
//! `#[ignore]`d — run it by hand when the chord stops working:
//! `cargo test -p nebula-tui --test claude_shift_enter_probe -- --ignored`.
#![cfg(windows)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const ROWS: u16 = 30;
const COLS: u16 = 100;

fn screen_lines(parser: &vt100::Parser) -> Vec<String> {
    (0..ROWS)
        .map(|row| {
            (0..COLS)
                .map(|col| {
                    parser
                        .screen()
                        .cell(row, col)
                        .map(|c| c.contents())
                        .unwrap_or_default()
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// Stand in for the daemon: feed output into the emulator and answer the
/// ConPTY cursor handshake, the kitty support query and DA1 — without those
/// replies the child never finishes booting, or never speaks kitty.
fn pump(
    rx: &std::sync::mpsc::Receiver<Vec<u8>>,
    parser: &mut vt100::Parser,
    writer: &mut Box<dyn Write + Send>,
    ms: u64,
) {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until {
        let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        parser.process(&chunk);
        let mut reply: Vec<u8> = Vec::new();
        if chunk.windows(4).any(|w| w == b"\x1b[6n") {
            reply.extend_from_slice(b"\x1b[1;1R");
        }
        if chunk.windows(4).any(|w| w == b"\x1b[?u") {
            reply.extend_from_slice(b"\x1b[?0u");
        }
        if chunk.windows(3).any(|w| w == b"\x1b[c") {
            reply.extend_from_slice(b"\x1b[?6c");
        }
        if !reply.is_empty() {
            let _ = writer.write_all(&reply).and_then(|_| writer.flush());
        }
    }
}

#[test]
#[ignore]
fn claude_takes_the_kitty_newline_chords() {
    let claude = nebula_core::spawn::resolve_program("claude").expect("claude on PATH");
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: ROWS,
            cols: COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut cmd = CommandBuilder::new(claude.display().to_string());
    cmd.cwd(std::env::current_dir().unwrap());
    cmd.env("TERM", "xterm-256color");
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

    let mut parser = vt100::Parser::new(ROWS, COLS, 0);
    pump(&rx, &mut parser, &mut writer, 8000);
    for step in [
        &b"abc"[..],
        b"\x1b[13;2u", // Shift+Enter
        b"def",
        b"\x1b[13;3u", // Alt+Enter
        b"ghi",
    ] {
        writer.write_all(step).unwrap();
        writer.flush().unwrap();
        pump(&rx, &mut parser, &mut writer, 1500);
    }

    let lines = screen_lines(&parser);
    let _ = child.kill();
    let prompt = lines
        .iter()
        .position(|l| l.contains("abc"))
        .unwrap_or_else(|| panic!("typing never reached the prompt:\n{}", lines.join("\n")));
    assert!(
        lines[prompt + 1].contains("def") && lines[prompt + 2].contains("ghi"),
        "a newline chord did not open a new line:\n{}",
        lines.join("\n")
    );
}
