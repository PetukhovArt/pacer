//! Rebuild `Event::Paste` from a burst of key events.
//!
//! Windows never delivers bracketed paste to a console app: crossterm reads
//! `INPUT_RECORD`s, and Windows Terminal injects a Ctrl+V paste as a stream
//! of synthetic keystrokes — every line break a bare Enter. Forwarded one by
//! one to the PTY, each Enter submits a message, so a pasted list lands in
//! the agent as many messages instead of one.
//!
//! Humans don't type multiple keys inside a few milliseconds; injected
//! pastes arrive in one `ReadConsoleInput` batch. The event loop drains
//! whatever arrived back-to-back and hands the batch to [`coalesce`], which
//! folds a multi-line run of plain keystrokes into the `Event::Paste` the
//! rest of the app already knows how to bracket.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Whether this event could open a paste burst — the event loop only pays
/// the drain window for these.
pub fn starts_burst(event: &Event) -> bool {
    matches!(event, Event::Key(key) if key.kind == KeyEventKind::Press && text_of(key).is_some())
}

/// Fold runs of plain keystrokes into `Event::Paste`. A run qualifies as a
/// paste only when it holds at least two presses and a line break — the
/// signature of injected multi-line text, and the only shape the key-by-key
/// path gets wrong (each Enter submits). Anything else passes through
/// untouched, in order.
pub fn coalesce(events: Vec<Event>) -> Vec<Event> {
    let mut out = Vec::new();
    // The presses of the current run, and every raw event backing it (so a
    // run that doesn't qualify replays verbatim, releases included).
    let mut text = String::new();
    let mut presses = 0usize;
    let mut raw: Vec<Event> = Vec::new();
    let flush =
        |text: &mut String, presses: &mut usize, raw: &mut Vec<Event>, out: &mut Vec<Event>| {
            if *presses >= 2 && text.contains('\n') {
                out.push(Event::Paste(std::mem::take(text)));
                raw.clear();
            } else {
                out.append(raw);
                text.clear();
            }
            *presses = 0;
        };
    for event in events {
        match &event {
            Event::Key(key) if text_of(key).is_some() => {
                if key.kind == KeyEventKind::Press {
                    text.push(text_of(key).unwrap());
                    presses += 1;
                }
                // A Release of a textual key rides along without breaking
                // the run — Windows injects down and up records alike.
                raw.push(event);
            }
            _ => {
                flush(&mut text, &mut presses, &mut raw, &mut out);
                out.push(event);
            }
        }
    }
    flush(&mut text, &mut presses, &mut raw, &mut out);
    out
}

/// The character a pasted keystroke stands for, or `None` when the key
/// carries chords/navigation and must never be folded into text.
fn text_of(key: &KeyEvent) -> Option<char> {
    let plain = key.modifiers.difference(KeyModifiers::SHIFT).is_empty();
    match key.code {
        KeyCode::Char(c) if plain => Some(c),
        KeyCode::Enter if plain => Some('\n'),
        KeyCode::Tab if key.modifiers.is_empty() => Some('\t'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn release(code: KeyCode) -> Event {
        let mut key = KeyEvent::new(code, KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        Event::Key(key)
    }

    fn typed(s: &str) -> Vec<Event> {
        s.chars()
            .map(|c| match c {
                '\n' => press(KeyCode::Enter),
                c => press(KeyCode::Char(c)),
            })
            .collect()
    }

    /// The reported bug: a pasted markdown list arrives as keystrokes with
    /// Enters between the items; it must fold into one paste.
    #[test]
    fn a_multiline_key_burst_becomes_one_paste() {
        let out = coalesce(typed("- one\n- two\n- three"));
        assert_eq!(out, vec![Event::Paste("- one\n- two\n- three".into())]);
    }

    #[test]
    fn a_single_line_burst_stays_keystrokes() {
        let events = typed("hello");
        assert_eq!(coalesce(events.clone()), events);
    }

    #[test]
    fn a_lone_enter_stays_a_keystroke() {
        let events = vec![press(KeyCode::Enter)];
        assert_eq!(coalesce(events.clone()), events);
    }

    /// Windows injects up records alongside downs; they ride the run without
    /// splitting it, and vanish once the run becomes a paste.
    #[test]
    fn release_events_do_not_split_the_run() {
        let events = vec![
            press(KeyCode::Char('a')),
            release(KeyCode::Char('a')),
            press(KeyCode::Enter),
            release(KeyCode::Enter),
            press(KeyCode::Char('b')),
        ];
        assert_eq!(coalesce(events), vec![Event::Paste("a\nb".into())]);
    }

    /// A chord in the middle breaks the run: what's before and after stands
    /// on its own, and neither half here qualifies as a paste.
    #[test]
    fn a_chord_passes_through_and_splits_the_run() {
        let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        let mut events = typed("ab");
        events.push(ctrl_c.clone());
        events.extend(typed("cd"));
        assert_eq!(coalesce(events.clone()), events);
    }

    #[test]
    fn tabs_survive_inside_a_paste() {
        let events = vec![
            press(KeyCode::Char('a')),
            press(KeyCode::Enter),
            press(KeyCode::Tab),
            press(KeyCode::Char('b')),
        ];
        assert_eq!(coalesce(events), vec![Event::Paste("a\n\tb".into())]);
    }

    #[test]
    fn only_textual_presses_open_a_burst() {
        assert!(starts_burst(&press(KeyCode::Char('x'))));
        assert!(starts_burst(&press(KeyCode::Enter)));
        assert!(!starts_burst(&release(KeyCode::Char('x'))));
        assert!(!starts_burst(&press(KeyCode::Esc)));
        assert!(!starts_burst(&Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        ))));
    }
}
