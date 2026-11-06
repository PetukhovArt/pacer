//! Does a wrapping Cyrillic paragraph survive ConPTY, and does our vt100
//! rebuild it?
//!
//! The artifact under investigation is a line that comes back with one
//! letter per word left standing at roughly the right columns — the shape a
//! column-width disagreement makes, not the shape lost bytes make. Three
//! layers could own it: the child, ConPTY's differential repaint, and the
//! `vt100` grid the TUI renders from. These probes cut the stack at ConPTY:
//! they assert the master reader sees the text, and that feeding exactly
//! those bytes to `vt100` at the same width reproduces every word.
//!
//! Cyrillic is the interesting alphabet here because U+0400..U+04FF is
//! East-Asian *Ambiguous*: `unicode-width` calls it 1, and a renderer that
//! calls it 2 lands every later glyph on the wrong column.

#![cfg(windows)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

const COLS: u16 = 60;
const ROWS: u16 = 24;

/// Long enough to wrap several times at 60 columns, and mixed the way agent
/// output is mixed: Cyrillic words, ASCII identifiers, and the ambiguous-width
/// punctuation (— « » …) that rides along with Russian prose.
const PARAGRAPH: &str = "совпадение и хунков, и заголовка с a1963f134 — это \
единственный вариант, где мерж двух линий по этому файлу становится \
тривиальным; атрибуция при этом остаётся честной, «фикс» действительно WC-2091…";

fn spawn_and_capture(ps_command: &str) -> Vec<u8> {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: ROWS,
            cols: COLS,
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
                    // The host repaints after the last write; give it a beat.
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

/// Print the paragraph as UTF-8 through the console, the way a Node agent CLI
/// does, and mark the end so the reader knows when to stop.
fn print_utf8(text: &str) -> String {
    format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
         [Console]::Write('{text}'); [Console]::Write('MARK-DONE')"
    )
}

/// The whole grid as lines, the way `replay_capture` prints it.
fn render(bytes: &[u8]) -> Vec<String> {
    let mut parser = vt100::Parser::new(ROWS, COLS, 0);
    parser.process(bytes);
    let screen = parser.screen();
    (0..ROWS)
        .map(|row| {
            (0..COLS)
                .filter_map(|col| screen.cell(row, col))
                .map(|c| {
                    if c.has_contents() {
                        c.contents().to_string()
                    } else {
                        " ".into()
                    }
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[test]
fn a_wrapping_cyrillic_paragraph_survives_conpty_and_vt100() {
    let out = spawn_and_capture(&print_utf8(PARAGRAPH));
    let raw = String::from_utf8_lossy(&out);
    assert!(
        raw.contains("MARK-DONE"),
        "the child never ran: {raw:?}\nbytes: {out:x?}"
    );

    // Wrapping means a word can straddle two rows, so compare on the grid
    // joined back up rather than row by row.
    let grid = render(&out);
    let joined: String = grid.join("");
    let flat: String = PARAGRAPH.chars().filter(|c| !c.is_whitespace()).collect();
    let seen: String = joined.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        seen.contains(&flat),
        "the paragraph did not come back whole.\n\
         want: {flat}\n\
         got:  {seen}\n\
         grid:\n{}",
        grid.join("\n")
    );
}

/// The artifact's signature: a redraw of the same line leaving one letter per
/// word behind. If ConPTY's repaint is what mangles Cyrillic, overwriting a
/// line in place (CR + rewrite, the way a spinner or a streaming answer does)
/// is where it shows.
#[test]
fn rewriting_a_cyrillic_line_in_place_leaves_no_stragglers() {
    let first = "первый вариант этой строки для проверки перерисовки";
    let second = "второй вариант этой строки для проверки перерисовки";
    let out = spawn_and_capture(&print_utf8(&format!(
        "{first}' + [char]13 + '{second}' + [char]13 + '{first}"
    )));
    let raw = String::from_utf8_lossy(&out);
    assert!(raw.contains("MARK-DONE"), "the child never ran: {raw:?}");

    let grid = render(&out);
    assert!(
        grid.iter().any(|l| l.contains(first)),
        "the last write did not land whole:\n{}",
        grid.join("\n")
    );
    assert!(
        !grid.iter().any(|l| l.contains(second)),
        "a stale line survived the overwrite:\n{}",
        grid.join("\n")
    );
}
