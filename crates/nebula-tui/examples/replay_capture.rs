//! Diagnosis: replay a captured PTY stream through the same vt100 the TUI
//! uses, and print the grid it lands on.
//!
//! Record with `NEBULA_PTY_CAPTURE=<dir> nebula daemon` (or a whole run),
//! reproduce the artifact, then:
//!
//!   cargo run -p nebula-tui --example replay_capture -- <dir>/agent-<id>.raw
//!
//! The `.meta` sidecar next to the `.raw` replays the resizes at the byte
//! offsets they happened on; `--cols/--rows` pins a size instead. `--upto N`
//! stops after N bytes, so a bisect can walk up to the frame that breaks.
//!
//! What the output settles: if the printed grid shows the artifact, the
//! mangling happened in *our* emulation of a stream that arrived intact. If
//! the grid is clean, the artifact lives further out — the ratatui buffer,
//! the outer terminal, or the capture missed a resize.
use std::io::Write;

struct Resize {
    at: u64,
    cols: u16,
    rows: u16,
}

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let (mut cols, mut rows, mut upto) = (None, None, u64::MAX);
    while let Some(arg) = args.next() {
        let mut val = || args.next().expect("flag wants a value");
        match arg.as_str() {
            "--cols" => cols = Some(val().parse().expect("cols")),
            "--rows" => rows = Some(val().parse().expect("rows")),
            "--upto" => upto = val().parse().expect("upto"),
            _ => path = Some(arg),
        }
    }
    let Some(path) = path else {
        eprintln!("usage: replay_capture <capture.raw> [--cols N --rows N] [--upto BYTES]");
        std::process::exit(2);
    };

    let raw = std::fs::read(&path)?;
    let resizes = match (cols, rows) {
        // An explicit size wins outright: replaying a stream at a width it
        // never ran at is exactly how a reflow bug shows itself.
        (Some(cols), Some(rows)) => vec![Resize { at: 0, cols, rows }],
        _ => read_meta(&path),
    };
    let (mut cols, mut rows) = match resizes.first() {
        Some(r) => (r.cols, r.rows),
        None => (80, 24),
    };
    let mut parser = vt100::Parser::new(rows, cols, 0);

    // Feed the stream in the same order the daemon saw it, breaking at each
    // recorded resize so the child's reflow lands where it originally did.
    let end = usize::try_from(upto.min(raw.len() as u64)).unwrap();
    let mut pos = 0usize;
    for next in resizes
        .iter()
        .skip(1)
        .map(|r| usize::try_from(r.at).unwrap().min(end))
        .chain([end])
    {
        if next > pos {
            parser.process(&raw[pos..next]);
            pos = next;
        }
        if let Some(r) = resizes.iter().find(|r| r.at as usize == pos && r.at > 0) {
            (cols, rows) = (r.cols, r.rows);
            parser.screen_mut().set_size(rows, cols);
        }
        if pos >= end {
            break;
        }
    }

    let screen = parser.screen();
    let mut out = std::io::stdout().lock();
    writeln!(out, "# {} bytes, {cols}x{rows}", end)?;
    for row in 0..rows {
        let mut line = String::new();
        for col in 0..cols {
            match screen.cell(row, col) {
                Some(c) if c.has_contents() => line.push_str(c.contents()),
                _ => line.push(' '),
            }
        }
        writeln!(out, "{row:>3}|{}|", line.trim_end())?;
    }
    Ok(())
}

/// Resizes from the `.meta` sidecar. Hand-parsed: the records are three
/// integers on a line, and a debug probe should not drag serde in for that.
fn read_meta(raw_path: &str) -> Vec<Resize> {
    let meta = std::path::Path::new(raw_path).with_extension("meta");
    let Ok(text) = std::fs::read_to_string(&meta) else {
        eprintln!("# no {} — defaulting to 80x24", meta.display());
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let num = |key: &str| -> Option<u64> {
                let rest = line.split_once(&format!("\"{key}\":"))?.1;
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().ok()
            };
            Some(Resize {
                at: num("at")?,
                cols: num("cols")? as u16,
                rows: num("rows")? as u16,
            })
        })
        .collect()
}
