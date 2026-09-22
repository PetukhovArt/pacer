//! The DIAGNOSTICS CARD: a non-modal floating panel listing every live
//! session's CPU, RAM and Claude token spend.
//!
//! It is **not** an `Overlay`. Every variant of that enum is modal and
//! centered, takes focus and swallows all input; this one coexists with an
//! open modal, swallows nothing but a drag of its own title row, and the
//! user moves it. So it is a plain field on `App`, which also means
//! GOTCHAS OVERLAY does not apply: `draw` takes `&mut App` and writes its
//! rect straight back, with no clone to write through. The reflex is to
//! copy the overlay pattern; don't.
//!
//! Position is a corner plus an offset from it, never an absolute `(x, y)`:
//! a position saved on a 200-column screen is offscreen on an 80-column
//! one. Anchoring to a corner keeps the card on screen with nothing to
//! clamp, which is what GOTCHAS SPLITTER DRAG warns about — never persist a
//! clamped value back.

use crate::app::{App, HitTarget};
use crate::price::PriceTable;
use pacer_core::SessionRef;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use serde::{Deserialize, Serialize};

/// Which screen corner the card is anchored to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Corner {
    #[default]
    TopRight,
    BottomRight,
    BottomLeft,
    TopLeft,
}

impl Corner {
    /// The four in cycling order, which is what the move key walks.
    fn next(self) -> Self {
        match self {
            Self::TopRight => Self::BottomRight,
            Self::BottomRight => Self::BottomLeft,
            Self::BottomLeft => Self::TopLeft,
            Self::TopLeft => Self::TopRight,
        }
    }

    fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    fn is_bottom(self) -> bool {
        matches!(self, Self::BottomRight | Self::BottomLeft)
    }
}

/// An in-progress drag of the card by its title row. `grab` is the cursor's
/// offset inside the card at mouse-down, so the card tracks the pointer
/// without jumping — the `SplitterDrag` pattern.
#[derive(Debug, Clone, Copy)]
pub struct CardDrag {
    grab: (u16, u16),
}

pub struct Card {
    pub shown: bool,
    pub corner: Corner,
    /// Cells in from that corner, x then y. Moved by dragging the title
    /// row; never negative once written.
    pub offset: (i16, i16),
    /// The whole card rect, written back during draw so clicks hit-test.
    /// Zero while hidden, which is what keeps `register_hit` quiet.
    pub area: Rect,
    pub drag: Option<CardDrag>,
    /// Rates for the cost column, from `config.json`. Empty unless the user
    /// filled `model_prices` in, and then the column is not drawn at all.
    pub prices: PriceTable,
    /// The screen the card was last placed against. A drag turns a cursor
    /// position into an offset from one of its corners, and the hit
    /// registration is the only thing that sees the frame.
    screen: Rect,
}

impl Default for Card {
    fn default() -> Self {
        Self {
            shown: false,
            corner: Corner::default(),
            offset: (1, 1),
            area: Rect::default(),
            drag: None,
            prices: PriceTable::default(),
            screen: Rect::default(),
        }
    }
}

impl Card {
    /// Both halves the config owns: whether the card starts on screen, and
    /// the rates its cost column needs.
    pub fn apply_config(&mut self, cfg: &crate::config::Config) {
        self.shown = cfg.show_diagnostics;
        self.prices = PriceTable::from_config(cfg);
    }

    pub fn cycle_corner(&mut self) {
        self.corner = self.corner.next();
        // A corner's offset is its own; starting a fresh one at the drag
        // offset of the last would push the card off two edges at once.
        self.offset = (1, 1);
    }
}

/// Show or hide the card. The caller persists the choice: a visibility
/// toggle is a setting, so it belongs in `config.json` rather than the
/// daemon's save-on-quit blob, which a crash can lose.
pub fn toggle(app: &mut App) {
    app.diagnostics.shown = !app.diagnostics.shown;
    if !app.diagnostics.shown {
        app.diagnostics.area = Rect::default();
        app.diagnostics.drag = None;
    }
}

/// Push the card's rect **before** the splitter grab zones.
///
/// GOTCHAS HIT TARGET: `hit_at` is first-match, and `ui::draw` registers
/// the splitters early so they beat panel rows. A card floating over a rule
/// has to beat the splitter, so its rect must go in first — registering
/// from inside `draw` would put it last and make every click over a rule a
/// silent no-op. The geometry is a pure function of the screen, the corner,
/// the offset and the row count, so it is knowable before anything is
/// drawn.
pub fn register_hit(app: &mut App, screen: Rect) {
    if !app.diagnostics.shown {
        // An undrawn card has a zero rect, and GOTCHAS CLICK OUTSIDE's
        // precheck depends on that staying true.
        app.diagnostics.area = Rect::default();
        return;
    }
    let area = geometry(app, screen);
    app.diagnostics.area = area;
    app.diagnostics.screen = screen;
    app.hits.push((area, HitTarget::DiagnosticsCard));
}

/// One rendered line of the card.
struct Row {
    name: String,
    cpu: Option<f32>,
    rss_bytes: u64,
    tokens: Option<pacer_core::TokenUsage>,
}

/// Where the card sits and how big it is. Clamped to the screen here and
/// only here: the clamp never travels back into `offset`.
fn geometry(app: &App, screen: Rect) -> Rect {
    let rows = rows(app).len() as u16;
    // header + rows + rule + total, inside a border.
    let height = (rows + 5).min(screen.height.max(1));
    let width = card_width(app).min(screen.width.max(1));

    let off_x = app.diagnostics.offset.0.max(0) as u16;
    let off_y = app.diagnostics.offset.1.max(0) as u16;
    let x = if app.diagnostics.corner.is_right() {
        screen
            .right()
            .saturating_sub(width)
            .saturating_sub(off_x)
            .max(screen.x)
    } else {
        screen
            .x
            .saturating_add(off_x)
            .min(screen.right().saturating_sub(width))
    };
    let y = if app.diagnostics.corner.is_bottom() {
        screen
            .bottom()
            .saturating_sub(height)
            .saturating_sub(off_y)
            .max(screen.y)
    } else {
        screen
            .y
            .saturating_add(off_y)
            .min(screen.bottom().saturating_sub(height))
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Column widths, fixed so the numbers line up between frames.
const W_NAME: usize = 16;
const W_CPU: usize = 5;
const W_RAM: usize = 6;
const W_TOK: usize = 6;
const W_COST: usize = 8;

fn card_width(app: &App) -> u16 {
    let cost = if app.diagnostics.prices.configured() {
        W_COST + 1
    } else {
        0
    };
    // Every column but the first carries a leading space, plus 2 for the
    // border.
    (W_NAME + (W_CPU + 1) + (W_RAM + 1) + 3 * (W_TOK + 1) + cost + 2) as u16
}

/// One row per live session, biggest first — the same order the metrics
/// modal uses.
fn rows(app: &App) -> Vec<Row> {
    let Some(snapshot) = &app.last_metrics else {
        return Vec::new();
    };
    let mut rows: Vec<Row> = snapshot
        .sessions
        .iter()
        .map(|m| {
            let name = match (&m.session, &m.prewarm) {
                // A pool spare has no row of its own to be named from.
                (_, Some(home)) => format!("warm {}", home.kind.as_str()),
                (SessionRef::Agent(id), None) => app
                    .tree
                    .agents
                    .iter()
                    .find(|a| &a.id == id)
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "(unknown)".into()),
                (SessionRef::Terminal(id), None) => app
                    .tree
                    .terminals
                    .iter()
                    .find(|t| &t.id == id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "(unknown)".into()),
            };
            Row {
                name,
                cpu: m.cpu_percent,
                rss_bytes: m.rss_bytes,
                tokens: m.tokens.clone(),
            }
        })
        .collect();
    rows.sort_by(|a, b| b.rss_bytes.cmp(&a.rss_bytes));
    rows
}

/// Token counts get short: 184k, 3.9M. Below a thousand they read exact.
fn fmt_tokens(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{}k", n / 1_000),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

/// `—` is "not measured" and `0` is "measured, nothing spent". The whole
/// design protects that distinction, so the card has to render it.
const UNMEASURED: &str = "—";

fn pad_left(s: &str, width: usize) -> String {
    let s = crate::ui::truncate(s, width);
    format!("{s:>width$}")
}

fn pad_right(s: &str, width: usize) -> String {
    let s = crate::ui::truncate(s, width);
    format!("{s:<width$}")
}

pub fn draw(f: &mut Frame, app: &mut App) {
    if !app.diagnostics.shown {
        return;
    }
    let th = app.theme;
    let area = geometry(app, f.area());
    app.diagnostics.area = area;
    app.diagnostics.screen = f.area();
    let inner = crate::ui::render_modal_frame(f, area, "Diagnostics", th);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let priced = app.diagnostics.prices.configured();
    let cell = |text: String, width: usize, right: bool| {
        if right {
            pad_left(&text, width)
        } else {
            pad_right(&text, width)
        }
    };
    let header = {
        let mut s = cell("SESSION".into(), W_NAME, false);
        s.push(' ');
        s.push_str(&cell("CPU".into(), W_CPU, true));
        s.push(' ');
        s.push_str(&cell("RAM".into(), W_RAM, true));
        s.push(' ');
        s.push_str(&cell("IN".into(), W_TOK, true));
        s.push(' ');
        s.push_str(&cell("OUT".into(), W_TOK, true));
        s.push(' ');
        s.push_str(&cell("CACHE".into(), W_TOK, true));
        if priced {
            s.push(' ');
            s.push_str(&cell("COST".into(), W_COST, true));
        }
        s
    };

    let rows = rows(app);
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        header,
        Style::default().fg(th.dim).add_modifier(Modifier::BOLD),
    ))];

    // The total sums what was measured; a session with no CPU reading is
    // not counted as idle, it is left out of the sum and the row says so.
    let mut any_cpu = false;
    let mut total_cpu = 0.0f32;
    let mut total_rss = 0u64;
    let mut total_tokens = pacer_core::TokenUsage::default();
    let mut any_tokens = false;

    for row in &rows {
        if let Some(cpu) = row.cpu {
            any_cpu = true;
            total_cpu += cpu;
        }
        total_rss += row.rss_bytes;
        if let Some(tokens) = &row.tokens {
            any_tokens = true;
            for model in &tokens.by_model {
                total_tokens.add(&model.model, model);
            }
        }
        lines.push(Line::from(Span::styled(
            render_row(row, priced, &app.diagnostics.prices),
            Style::default().fg(th.text),
        )));
    }
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "no live sessions",
            Style::default().fg(th.dim),
        )));
    }

    lines.push(Line::from(Span::styled(
        "─".repeat(inner.width as usize),
        Style::default().fg(th.dim),
    )));
    let total = Row {
        name: "total".into(),
        cpu: any_cpu.then_some(total_cpu),
        rss_bytes: total_rss,
        tokens: any_tokens.then_some(total_tokens),
    };
    lines.push(Line::from(Span::styled(
        render_row(&total, priced, &app.diagnostics.prices),
        Style::default().fg(th.muted).add_modifier(Modifier::BOLD),
    )));

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_row(row: &Row, priced: bool, prices: &PriceTable) -> String {
    let (input, output, cache) = match &row.tokens {
        Some(tokens) => {
            let (i, o, c) = tokens.totals();
            (fmt_tokens(i), fmt_tokens(o), fmt_tokens(c))
        }
        None => (UNMEASURED.into(), UNMEASURED.into(), UNMEASURED.into()),
    };
    let cpu = match row.cpu {
        Some(cpu) => format!("{cpu:.0}%"),
        None => UNMEASURED.into(),
    };
    let mut s = pad_right(&row.name, W_NAME);
    s.push(' ');
    s.push_str(&pad_left(&cpu, W_CPU));
    s.push(' ');
    s.push_str(&pad_left(&crate::ui::fmt_mem(row.rss_bytes), W_RAM));
    s.push(' ');
    s.push_str(&pad_left(&input, W_TOK));
    s.push(' ');
    s.push_str(&pad_left(&output, W_TOK));
    s.push(' ');
    s.push_str(&pad_left(&cache, W_TOK));
    if priced {
        // `$0.00` is unrepresentable for an unpriced model: `cost_usd`
        // answers None and there is no other arithmetic to fall back on.
        let cost = row
            .tokens
            .as_ref()
            .and_then(|t| prices.cost_usd(t))
            .map(|usd| format!("${usd:.2}"))
            .unwrap_or_else(|| UNMEASURED.into());
        s.push(' ');
        s.push_str(&pad_left(&cost, W_COST));
    }
    s
}

/// Every mouse gesture the card owns, settled before any overlay sees the
/// event. Returns whether it was consumed.
///
/// One entry point and not three, because the card floats above modals:
/// routing it through the usual per-kind arms would make a click on it
/// count as a click outside whatever modal is open, and a drag over a rule
/// arm the splitter underneath.
pub fn on_mouse(app: &mut App, mouse: &crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if app.diagnostics.area.width == 0
                || app.hit_at(mouse.column, mouse.row) != Some(HitTarget::DiagnosticsCard)
            {
                return false;
            }
            on_press(app, mouse.column, mouse.row);
            true
        }
        MouseEventKind::Drag(MouseButton::Left) => on_drag(app, mouse.column, mouse.row),
        MouseEventKind::Up(MouseButton::Left) => on_release(app),
        // The card has nothing to scroll, but it covers whatever does, so
        // swallow the wheel rather than let it move a list the user cannot
        // see. The base panels hit-test for themselves; the modals scroll
        // unconditionally, which is what this is for.
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            app.diagnostics.area.width > 0
                && app.hit_at(mouse.column, mouse.row) == Some(HitTarget::DiagnosticsCard)
        }
        _ => false,
    }
}

/// Mouse-down inside the card. Only the title row (its top border) arms a
/// drag; the rest of the card swallows the click and does nothing else.
fn on_press(app: &mut App, x: u16, y: u16) {
    let area = app.diagnostics.area;
    if area.width == 0 || y != area.y {
        return;
    }
    app.diagnostics.drag = Some(CardDrag {
        grab: (x.saturating_sub(area.x), y.saturating_sub(area.y)),
    });
}

/// Returns whether the drag was the card's.
fn on_drag(app: &mut App, x: u16, y: u16) -> bool {
    let Some(drag) = app.diagnostics.drag else {
        return false;
    };
    let screen = app.diagnostics.screen;
    let area = app.diagnostics.area;
    let top_left_x = x.saturating_sub(drag.grab.0);
    let top_left_y = y.saturating_sub(drag.grab.1);
    // Measured from whichever corner the card is anchored to, so the
    // stored number stays meaningful on a differently sized screen.
    let off_x = if app.diagnostics.corner.is_right() {
        screen.right() as i32 - (top_left_x + area.width) as i32
    } else {
        top_left_x as i32 - screen.x as i32
    };
    let off_y = if app.diagnostics.corner.is_bottom() {
        screen.bottom() as i32 - (top_left_y + area.height) as i32
    } else {
        top_left_y as i32 - screen.y as i32
    };
    app.diagnostics.offset = (off_x.max(0) as i16, off_y.max(0) as i16);
    app.dirty = true;
    true
}

/// Returns whether a card drag was the thing that ended.
fn on_release(app: &mut App) -> bool {
    app.diagnostics.drag.take().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 40,
    };

    fn shown_app() -> App {
        let mut app = App::new();
        app.diagnostics.shown = true;
        app
    }

    /// The card is registered before the splitter grab zones, so a click
    /// over a rule it floats on reaches the card. GOTCHAS HIT TARGET:
    /// `hit_at` is first-match, so the wrong order makes this a silent
    /// no-op rather than a visible bug.
    #[test]
    fn the_card_beats_a_splitter_zone_it_overlaps() {
        let mut app = shown_app();
        register_hit(&mut app, SCREEN);
        let area = app.diagnostics.area;
        assert!(area.width > 0, "a shown card registers a rect");

        // A rule running down the middle of the card.
        let rule_x = area.x + area.width / 2;
        app.hits
            .push((Rect::new(rule_x, 0, 2, 40), HitTarget::Splitter(0)));

        assert_eq!(
            app.hit_at(rule_x, area.y + 1),
            Some(HitTarget::DiagnosticsCard)
        );
        // Off the card, the splitter still wins its own cells.
        assert_eq!(
            app.hit_at(rule_x, area.bottom() + 1),
            Some(HitTarget::Splitter(0))
        );
    }

    #[test]
    fn a_hidden_card_registers_nothing() {
        let mut app = shown_app();
        app.diagnostics.shown = false;
        register_hit(&mut app, SCREEN);
        assert_eq!(app.diagnostics.area, Rect::default());
        assert!(app.hits.is_empty());
    }

    /// Press on the title row, drag, release: the offset follows the
    /// cursor and the drag ends. Catches a drag that jumps the card to the
    /// cursor rather than tracking the grab point.
    #[test]
    fn dragging_the_title_row_moves_the_card() {
        let mut app = shown_app();
        register_hit(&mut app, SCREEN);
        let area = app.diagnostics.area;
        let before = app.diagnostics.offset;

        // Anywhere but the title row does not arm a drag.
        on_press(&mut app, area.x + 2, area.y + 2);
        assert!(app.diagnostics.drag.is_none());
        assert!(!on_drag(&mut app, area.x + 2, area.y + 5));

        on_press(&mut app, area.x + 2, area.y);
        assert!(app.diagnostics.drag.is_some());
        // Three cells left and two down, from a TopRight anchor: the x
        // offset grows (further from the right edge), the y offset too.
        assert!(on_drag(&mut app, area.x - 1, area.y + 2));
        assert_eq!(
            app.diagnostics.offset,
            (before.0 + 3, before.1 + 2),
            "the card tracks the grab point, it does not jump to the cursor"
        );
        assert!(on_release(&mut app));
        assert!(!on_release(&mut app), "the release is consumed once");
    }

    #[test]
    fn cycling_a_corner_walks_all_four_and_returns() {
        let mut card = Card::default();
        assert_eq!(card.corner, Corner::TopRight);
        let walk: Vec<Corner> = (0..4)
            .map(|_| {
                card.cycle_corner();
                card.corner
            })
            .collect();
        assert_eq!(
            walk,
            vec![
                Corner::BottomRight,
                Corner::BottomLeft,
                Corner::TopLeft,
                Corner::TopRight
            ]
        );
    }
}
