//! Does the pane ratatui draws survive a *real* ConPTY hop?
//!
//! `cyrillic_render_probe` closes the ratatui-diff loop against a vt100
//! standing in for the outer terminal. But when the TUI itself runs under
//! ConPTY (an IDE terminal, ssh through OpenSSH, `pacer` inside another
//! multiplexer), the outer arbiter of glyph widths is ConPTY's own
//! emulation — and any cell it sizes differently from `unicode-width`
//! desynchronises ratatui's diff on every later frame.
//!
//! The probe: draw frames through a real `CrosstermBackend`, hand the
//! emitted bytes to a child running inside a real ConPTY, read back what
//! ConPTY re-emits, and compare that grid (via vt100) against ratatui's
//! buffer.

#![cfg(windows)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::rc::Rc;
use std::time::{Duration, Instant};
use tui_term::widget::PseudoTerminal;

const COLS: u16 = 60;
const ROWS: u16 = 8;
/// The pty is one row taller than the pane so the DONE marker has a home
/// row that never collides with pane content.
const PTY_ROWS: u16 = ROWS + 1;

#[derive(Clone, Default)]
struct Tap(Rc<RefCell<Vec<u8>>>);

impl std::io::Write for Tap {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One frame of pane content, as the bytes a child would write.
fn frame(lines: &[&str]) -> Vec<u8> {
    let mut out = b"\x1b[2J\x1b[H".to_vec();
    for (i, line) in lines.iter().enumerate() {
        out.extend_from_slice(format!("\x1b[{};1H", i + 1).as_bytes());
        out.extend_from_slice(line.as_bytes());
    }
    out
}

fn buffer_grid(buf: &ratatui::buffer::Buffer) -> Vec<String> {
    (0..ROWS)
        .map(|row| {
            (0..COLS)
                .map(|col| buf[(col, row)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// Replay `chunks` inside a real ConPTY, one write + pause per chunk, and
/// return everything ConPTY re-emitted once the marker landed.
fn conpty_replay(chunks: &[Vec<u8>]) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    for (i, chunk) in chunks.iter().enumerate() {
        std::fs::write(dir.path().join(format!("frame{i}.bin")), chunk).unwrap();
    }
    // Raw byte replay: ConPTY decodes WriteFile output with the console
    // output code page, so pin 65001 the way agent CLIs do.
    let script = format!(
        "$null = chcp 65001; \
         $out = [Console]::OpenStandardOutput(); \
         0..{last} | ForEach-Object {{ \
           $b = [IO.File]::ReadAllBytes((Join-Path '{dir}' (\"frame$_.bin\"))); \
           $out.Write($b, 0, $b.Length); $out.Flush(); \
           Start-Sleep -Milliseconds 60 \
         }}; \
         $m = [byte[]](@(27) + [Text.Encoding]::ASCII.GetBytes('[{mrow};1HMARK-DONE')); \
         $out.Write($m, 0, $m.Length); $out.Flush(); \
         Start-Sleep -Milliseconds 400",
        last = chunks.len() - 1,
        dir = dir.path().display(),
        mrow = PTY_ROWS,
    );

    let pty = native_pty_system()
        .openpty(PtySize {
            rows: PTY_ROWS,
            cols: COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut cmd = CommandBuilder::new("powershell.exe");
    cmd.args(["-NoProfile", "-Command", &script]);
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
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => {
                out.extend_from_slice(&chunk);
                if !answered_dsr && out.windows(4).any(|w| w == b"\x1b[6n") {
                    writer.write_all(b"\x1b[1;1R").unwrap();
                    writer.flush().unwrap();
                    answered_dsr = true;
                }
                if out.windows(9).any(|w| w == b"MARK-DONE") {
                    std::thread::sleep(Duration::from_millis(300));
                    while let Ok(chunk) = rx.try_recv() {
                        out.extend_from_slice(&chunk);
                    }
                    break;
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

/// The pane rows of the grid ConPTY's re-emission lands on.
fn outer_grid(bytes: &[u8]) -> Vec<String> {
    let mut parser = vt100::Parser::new(PTY_ROWS, COLS, 0);
    parser.process(bytes);
    let screen = parser.screen();
    (0..ROWS)
        .map(|row| {
            (0..COLS)
                .map(|col| match screen.cell(row, col) {
                    Some(c) if c.has_contents() => c.contents().to_string(),
                    _ => " ".to_string(),
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// Draw every frame through a real backend, ship the emitted bytes through a
/// real ConPTY, and assert the settled grid matches ratatui's last buffer.
fn drive(frames: &[Vec<&str>]) {
    let tap = Tap::default();
    let mut terminal = Terminal::new(CrosstermBackend::new(tap.clone())).unwrap();

    let mut child = vt100::Parser::new(ROWS, COLS, 0);
    let mut consumed = 0usize;
    let mut chunks = Vec::new();
    let mut drawn = Vec::new();

    for lines in frames {
        child.process(&frame(lines));
        terminal
            .draw(|f| {
                let widget = PseudoTerminal::new(child.screen());
                f.render_widget(widget, f.area());
                drawn = buffer_grid(f.buffer_mut());
            })
            .unwrap();
        let out = tap.0.borrow();
        chunks.push(out[consumed..].to_vec());
        consumed = out.len();
    }

    let echoed = conpty_replay(&chunks);
    assert!(
        echoed.windows(9).any(|w| w == b"MARK-DONE"),
        "the replay child never finished: {:?}",
        String::from_utf8_lossy(&echoed)
    );
    assert_eq!(
        outer_grid(&echoed),
        drawn,
        "ConPTY's re-emission drifted from what ratatui drew.\n\
         conpty emitted: {:?}",
        String::from_utf8_lossy(&echoed)
    );
}

#[test]
fn a_static_cyrillic_pane_survives_a_conpty_hop() {
    drive(&[vec![
        "совпадение и хунков, и заголовка с a1963f134 это",
        "единственный вариант, где мерж двух линий по",
        "этому файлу становится тривиальным; атрибуция",
    ]]);
}

#[test]
fn redrawing_a_cyrillic_pane_survives_a_conpty_hop() {
    drive(&[
        vec!["совпадение и хунков, и заголовка с a1963f134 это"],
        vec!["совпадение и хунков, и заголовка с a1963f134 эти"],
        vec!["совпадение и хунков; и заголовка с a1963f135 это"],
        vec!["1 — совпадение и хунков, и заголовка с a1963f134"],
        vec![""],
        vec!["атрибуция при этом остаётся честной, «фикс» — да"],
    ]);
}

#[test]
fn ambiguous_width_punctuation_survives_a_conpty_hop() {
    drive(&[
        vec!["✳ Baked for 1m 17s · done 1:55 PM", "└ Tip: —«»…→ ✓"],
        vec!["✳ Baked for 2m 17s · done 1:56 PM", "└ Tip: —«»…→ ✓"],
    ]);
}
