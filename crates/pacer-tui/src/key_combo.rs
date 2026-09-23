//! The KEY COMBO DISPLAY (Settings › Experimental, `show_key_combos`):
//! each key pressed in the panels, spelled at the bottom left of the
//! screen with what it did (`j - Move down`, `h h - Workspaces`), so
//! someone watching a screen share can pick the shortcuts up. Modelled on
//! vim's `showcmd`: the combo clears itself [`LINGER`] after the press.
//!
//! What it never shows: keys typed into a LOCKED PANE (they are the
//! agent's, a password included; only the unlock hatch shows) and text
//! typed into an overlay. Inside a modal only keys that cannot be text
//! show, bare ([`is_text_key`] draws the line).

use crate::app::App;
use crate::keymap::KeyChord;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::{Duration, Instant};

/// How long the last combo stays on screen after its press.
pub const LINGER: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCombo {
    /// The press, or both presses of a double tap, in order.
    pub keys: Vec<KeyChord>,
    /// What it did: the action's label. None for a key that fired nothing.
    pub does: Option<String>,
    pub at: Instant,
}

/// Time left before the loop takes the combo down; zero when none is up.
pub fn remaining(app: &App) -> Duration {
    app.key_combo
        .as_ref()
        .map(|c| (c.at + LINGER).saturating_duration_since(Instant::now()))
        .unwrap_or_default()
}

/// Record a press. A no-op while the setting is off, so callers need not
/// check; the newest press replaces the last.
pub fn note(app: &mut App, keys: &[KeyChord], does: Option<&str>) {
    if !app.show_key_combos || keys.is_empty() {
        return;
    }
    app.key_combo = Some(KeyCombo {
        keys: keys.to_vec(),
        does: does.map(str::to_string),
        at: Instant::now(),
    });
    app.dirty = true;
}

/// A press in the panels: the key and the label of the action it fired.
pub fn note_action(app: &mut App, chord: KeyChord, action: Option<crate::keymap::Action>) {
    let label = action
        .and_then(crate::keymap::index_of)
        .and_then(crate::keymap::spec_at)
        .map(|s| s.label);
    note(app, &[chord], label);
}

/// The second press of a double tap, shown as one combo worded from the
/// footer's `h again: …` hint.
pub fn note_double_tap(app: &mut App, chord: &KeyChord, does: &str) {
    let mut chars = does.chars();
    let label: String = chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    note(app, &[*chord, *chord], Some(&label));
}

/// A chord an overlay takes as typed text rather than as a command: a
/// printable character, shifted or not, or an edit to one. A ^ or ⌥ chord
/// is a command everywhere and shows.
pub fn is_text_key(chord: &KeyChord) -> bool {
    let command_mods = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER;
    !chord.mods.intersects(command_mods)
        && matches!(
            chord.code,
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
        )
}

/// Draw the combo on the footer's blank padding row, far left: the row
/// above the bar, so the bar's fixed column budget is untouched. Each key
/// sits in a keycap (the selected-row fill); the label is plain text.
pub fn draw(f: &mut Frame, app: &App, footer: Rect) {
    let Some(combo) = &app.key_combo else {
        return;
    };
    if footer.height < 2 || footer.width == 0 {
        return;
    }
    let row = Rect {
        height: 1,
        ..footer
    };
    let th = app.theme;
    let cap = Style::default()
        .fg(th.accent)
        .bg(th.sel_bg)
        .add_modifier(Modifier::BOLD);
    let mut spans = vec![Span::raw(" ")];
    for (i, key) in combo.keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(format!(" {} ", key.display()), cap));
    }
    if let Some(does) = &combo.does {
        spans.push(Span::styled(" - ", Style::default().fg(th.dim)));
        spans.push(Span::styled(does.as_str(), Style::default().fg(th.text)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), row);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;

    fn chord(code: KeyCode, mods: KeyModifiers) -> KeyChord {
        KeyChord::from_event(&KeyEvent::new(code, mods))
    }

    /// Inside a modal, what is typed is text and must stay off the screen
    /// (a filter, a name, an ssh destination); navigation keys and ^/⌥
    /// chords show.
    #[test]
    fn text_keys_are_printable_characters_and_their_edits() {
        for c in [
            chord(KeyCode::Char('a'), KeyModifiers::NONE),
            chord(KeyCode::Char('A'), KeyModifiers::SHIFT),
            chord(KeyCode::Char(' '), KeyModifiers::NONE),
            chord(KeyCode::Backspace, KeyModifiers::NONE),
        ] {
            assert!(is_text_key(&c), "{} is text", c.display());
        }
        for c in [
            chord(KeyCode::Esc, KeyModifiers::NONE),
            chord(KeyCode::Down, KeyModifiers::NONE),
            chord(KeyCode::Char('u'), KeyModifiers::CONTROL),
            chord(KeyCode::Char('p'), KeyModifiers::ALT),
        ] {
            assert!(!is_text_key(&c), "{} is a key", c.display());
        }
    }
}
