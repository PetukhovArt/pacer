//! The ConPTY platform assumptions the key-encoding stack stands on, pinned
//! as tests against the real ConPTY. Three are load-bearing:
//!
//! 1. Kitty keyboard negotiation in the child's *output* passes through
//!    ConPTY to the master reader — that is where `pty::kitty::KittyScanner`
//!    listens, so if a Windows update started swallowing `CSI > 1 u` the
//!    kitty dialect would silently die for every agent CLI.
//! 2. A legacy control byte written to the master *input* is translated into
//!    a proper `INPUT_RECORD` for a cooked child (Ctrl+U reaches PSReadLine).
//! 3. A win32-input-mode record (`CSI Vk;Sc;Uc;Kd;Cs;Rc _`) is accepted on
//!    input and reconstructed with its modifiers — the only route by which a
//!    cooked child ever sees Shift+Enter, and what `keys::encode_win32`
//!    emits.
//!
//! A fourth pins what the *outer* terminal can inject: conhost drops kitty
//! CSI-u typed at a console app, so a Windows Terminal `sendInput` binding
//! for Shift+Enter silently swallows the chord before nebula sees it.
//!
//! The two `#[ignore]`d probes need tools off PATH (node, claude) and exist
//! for hand-run diagnosis, not the grid.

#![cfg(windows)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// Spawn `powershell -Command <ps_command>` under a fresh ConPTY, answer the
/// `INHERIT_CURSOR` handshake, optionally write `input_after_ready` once the
/// child has printed READY, and return everything the master reader saw.
fn spawn_and_capture(ps_command: &str, input_after_ready: Option<&[u8]>) -> Vec<u8> {
    spawn_and_capture_for(ps_command, input_after_ready, 15)
}

/// As above, with the read window in seconds — a probe that expects *no*
/// key to arrive waits out the whole window, so it asks for a short one.
fn spawn_and_capture_for(
    ps_command: &str,
    input_after_ready: Option<&[u8]>,
    window_secs: u64,
) -> Vec<u8> {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut cmd = CommandBuilder::new("powershell.exe");
    cmd.args(["-NoProfile", "-Command", ps_command]);
    let mut child = pty.slave.spawn_command(cmd).unwrap();
    drop(pty.slave);
    let mut reader = pty.master.try_clone_reader().unwrap();
    let mut writer = pty.master.take_writer().unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    let mut out = Vec::new();
    let mut answered_dsr = false;
    let mut sent_input = input_after_ready.is_none();
    let deadline = Instant::now() + Duration::from_secs(window_secs);
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => {
                out.extend_from_slice(&chunk);
                // The INHERIT_CURSOR handshake: whoever reads the master must
                // answer the host's ESC[6n or the child never runs.
                if !answered_dsr && out.windows(4).any(|w| w == b"\x1b[6n") {
                    writer.write_all(b"\x1b[1;1R").unwrap();
                    writer.flush().unwrap();
                    answered_dsr = true;
                }
                if !sent_input && out.windows(5).any(|w| w == b"READY") {
                    writer.write_all(input_after_ready.unwrap()).unwrap();
                    writer.flush().unwrap();
                    sent_input = true;
                }
            }
            Err(_) => {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
            }
        }
    }
    let _ = child.kill();
    out
}

/// A child that prints READY, reads one key through the cooked Win32 console
/// API, and reports what it saw as `KEY:<char code>:<key>:<modifiers>`.
const READS_ONE_KEY: &str = "[Console]::Write('READY'); $k=[Console]::ReadKey($true); \
     [Console]::Write('KEY:' + [int]$k.KeyChar + ':' + $k.Key + ':' + $k.Modifiers)";

#[test]
fn kitty_negotiation_in_child_output_survives_conpty() {
    let out = spawn_and_capture(
        "[Console]::Write([char]27 + '[>1u' + [char]27 + '[?u' + 'MARK-DONE')",
        None,
    );
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("MARK-DONE"), "the child never ran: {text:?}");
    assert!(
        text.contains("\x1b[>1u"),
        "ConPTY ate the kitty push — the KittyScanner is blind: {text:?}"
    );
    assert!(
        text.contains("\x1b[?u"),
        "ConPTY ate the kitty query — children can't detect support: {text:?}"
    );
}

#[test]
fn a_legacy_ctrl_byte_reaches_a_cooked_child_as_a_key_record() {
    let out = spawn_and_capture(READS_ONE_KEY, Some(b"\x15"));
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("KEY:21:U:Control"),
        "0x15 did not arrive as Ctrl+U: {text:?}"
    );
}

/// What `keys::encode_win32` emits for Shift+Enter, accepted end to end.
#[test]
fn a_win32_input_record_carries_shift_enter_to_a_cooked_child() {
    let out = spawn_and_capture(
        READS_ONE_KEY,
        Some(b"\x1b[13;0;13;1;16;1_\x1b[13;0;13;0;16;1_"),
    );
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("KEY:13:Enter:Shift"),
        "the record pair did not arrive as Shift+Enter: {text:?}"
    );
}

/// Hand-run diagnosis: a raw-VT child (node, like every agent CLI) must see
/// kitty CSI-u input byte-for-byte. Needs node on PATH.
#[test]
#[ignore]
fn probe_input_csi_u_raw_mode_node() {
    let script = "process.stdout.write('READY'); process.stdin.setRawMode(true); process.stdin.on('data', d => { process.stdout.write('BYTES:' + JSON.stringify([...d])); process.exit(0); });";
    let out = spawn_and_capture(
        &format!("node -e \"{}\"", script.replace('"', "`\"")),
        Some(b"\x1b[13;2u"),
    );
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("BYTES:[27,91,49,51,59,50,117]"),
        "CSI-u did not survive to a raw-mode child: {text:?}"
    );
}

/// Hand-run diagnosis: does the real Claude Code CLI negotiate kitty under
/// ConPTY? (Observed 2026-08: it does — `CSI < u` then `CSI > 5 u`.)
#[test]
#[ignore]
fn probe_claude_kitty_negotiation() {
    let out = spawn_and_capture("claude", None);
    let text = String::from_utf8_lossy(&out);
    eprintln!("== claude output ({} bytes) ==", out.len());
    for probe in ["\x1b[?u", "\x1b[>", "\x1b[=", "\x1b[<"] {
        eprintln!("contains {:?}: {}", probe, text.contains(probe));
    }
    eprintln!("{}", text.escape_debug());
}

/// What a *console app* — nebula's own TUI, which reads `INPUT_RECORD`s
/// through crossterm — sees when the outer terminal injects a chord as
/// text, the way a Windows Terminal `sendInput` binding does.
///
/// Kitty CSI-u dies at conhost before any key record exists (a raw-VT child
/// would have received it verbatim — see `probe_input_csi_u_raw_mode_node`),
/// so a `sendInput "\\u001b[13;2u"` binding, the recipe for running an agent
/// CLI straight in Windows Terminal, makes Shift+Enter vanish on the way
/// into nebula. `ESC CR` survives as Alt+Enter, which the TUI re-encodes
/// into the child's own dialect — that is the binding to keep.
#[test]
fn injected_csi_u_dies_at_conhost_while_esc_cr_becomes_alt_enter() {
    let out = spawn_and_capture_for(READS_ONE_KEY, Some(b"\x1b[13;2u"), 4);
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("READY") && !text.contains("KEY:"),
        "conhost handed CSI-u to a console app after all: {text:?}"
    );

    let out = spawn_and_capture(READS_ONE_KEY, Some(b"\x1b\r"));
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("KEY:13:Enter:Alt"),
        "ESC CR did not arrive as Alt+Enter: {text:?}"
    );
}
