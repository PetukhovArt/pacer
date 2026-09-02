//! Diagnosis: what does this terminal actually deliver to crossterm?
//!
//! Run it in the terminal you launch pacer from and press the chord under
//! suspicion (Shift+Enter, Alt+Enter). Each line is one event as the TUI's
//! `handle_key` would see it, plus the bytes `keys::encode_key` would send
//! to a kitty-speaking child (Claude Code pushes flags 5) and to a legacy
//! one. Ctrl+C quits.
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::Write;

fn main() -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut out = std::io::stdout();
    write!(
        out,
        "press chords (Shift+Enter, Alt+Enter, ...), Ctrl+C to quit\r\n"
    )?;
    out.flush()?;
    loop {
        match event::read()? {
            Event::Key(key) => {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
                let kitty = pacer_tui::keys::encode_key(&key, 5, false);
                let legacy = pacer_tui::keys::encode_key(&key, 0, false);
                write!(
                    out,
                    "{:?} mods={:?} kind={:?} -> kitty {:?} | legacy {:?}\r\n",
                    key.code,
                    key.modifiers,
                    key.kind,
                    kitty.map(|b| String::from_utf8_lossy(&b).escape_debug().to_string()),
                    legacy.map(|b| String::from_utf8_lossy(&b).escape_debug().to_string()),
                )?;
                out.flush()?;
            }
            Event::Resize(..) | Event::FocusGained | Event::FocusLost => {}
            other => {
                write!(out, "{other:?}\r\n")?;
                out.flush()?;
            }
        }
    }
    disable_raw_mode()
}
