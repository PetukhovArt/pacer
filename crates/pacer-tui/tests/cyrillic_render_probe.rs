//! Does the pane the TUI *draws* match the pane the outer terminal *shows*?
//!
//! Between the two sits ratatui's frame diff, which re-emits only the cells
//! it believes changed and skips the cursor move whenever the next cell is
//! the one after the last. Both shortcuts stand on ratatui and the terminal
//! agreeing on how many columns a glyph takes. Cyrillic is East-Asian
//! *Ambiguous*, so it is exactly where that agreement is worth pinning: one
//! column of drift on one frame turns into stale glyphs scattered across
//! later frames, because the diff never revisits cells it thinks are fine.
//!
//! The probe closes the loop: drive real frames through a real
//! `CrosstermBackend`, replay the escape sequences it emitted into a second
//! `vt100`, and compare that grid against ratatui's own buffer.

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::cell::RefCell;
use std::rc::Rc;
use tui_term::widget::PseudoTerminal;

const COLS: u16 = 60;
const ROWS: u16 = 8;

/// The backend's writer, kept readable from outside it — ratatui does not
/// hand its writer back (`writer_mut` is unstable), and the emitted bytes are
/// the whole point of this probe.
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

/// The grid a vt100 lands on after `bytes`, trimmed for comparison.
fn grid(parser: &vt100::Parser) -> Vec<String> {
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

/// What ratatui believes it put on screen.
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

/// Draw every frame through a real backend, replaying what it emitted into a
/// terminal emulator, and assert the two agree after each one.
fn drive(frames: &[Vec<&str>]) {
    let tap = Tap::default();
    let mut terminal = Terminal::new(CrosstermBackend::new(tap.clone())).unwrap();

    let mut child = vt100::Parser::new(ROWS, COLS, 0);
    let mut outer = vt100::Parser::new(ROWS, COLS, 0);
    let mut consumed = 0usize;

    for (n, lines) in frames.iter().enumerate() {
        child.process(&frame(lines));
        // Snapshot inside the closure: `draw` swaps buffers on the way out,
        // so afterwards `current_buffer_mut` is the *next* frame's blank one.
        let mut drawn = Vec::new();
        terminal
            .draw(|f| {
                let widget = PseudoTerminal::new(child.screen());
                f.render_widget(widget, f.area());
                drawn = buffer_grid(f.buffer_mut());
            })
            .unwrap();

        // Everything the backend emitted for this frame, through an emulator:
        // that is what the user's terminal is showing.
        let emitted = {
            let out = tap.0.borrow();
            let chunk = out[consumed..].to_vec();
            consumed = out.len();
            chunk
        };
        outer.process(&emitted);

        assert_eq!(
            grid(&outer),
            drawn,
            "frame {n}: what the terminal shows drifted from what ratatui drew\n\
             emitted: {:?}",
            String::from_utf8_lossy(&emitted)
        );
    }
}

#[test]
fn a_static_cyrillic_pane_reaches_the_terminal_intact() {
    drive(&[vec![
        "совпадение и хунков, и заголовка с a1963f134 это",
        "единственный вариант, где мерж двух линий по",
        "этому файлу становится тривиальным; атрибуция",
    ]]);
}

/// The artifact's shape: the same sentence redrawn, one letter per word left
/// standing. That is a diff that skipped cells it should have rewritten, so
/// the frames here change a little at a time — the case the diff optimises.
#[test]
fn redrawing_a_cyrillic_pane_leaves_nothing_stale() {
    drive(&[
        vec!["совпадение и хунков, и заголовка с a1963f134 это"],
        vec!["совпадение и хунков, и заголовка с a1963f134 эти"],
        vec!["совпадение и хунков; и заголовка с a1963f135 это"],
        vec!["1 — совпадение и хунков, и заголовка с a1963f134"],
        vec![""],
        vec!["атрибуция при этом остаётся честной, «фикс» — да"],
    ]);
}

/// Mixed with the ambiguous-width punctuation agent output actually carries.
#[test]
fn ambiguous_width_punctuation_reaches_the_terminal_intact() {
    drive(&[
        vec!["✳ Baked for 1m 17s · done 1:55 PM", "└ Tip: —«»…→ ✓"],
        vec!["✳ Baked for 2m 17s · done 1:56 PM", "└ Tip: —«»…→ ✓"],
    ]);
}
