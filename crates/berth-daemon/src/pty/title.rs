//! OSC 0/2 window-title scanner — the end-of-turn signal that is left once
//! a Claude Code stops emitting OSC 9;4.
//!
//! `pty::progress` explains why an end-of-turn signal must come off the PTY
//! at all: a cancelled turn fires no `Stop` hook, and the `idle_prompt`
//! notification that would otherwise un-stick it is suppressed because the
//! user just pressed a key. Claude Code 2.1.270 emits no OSC 9;4 at all —
//! measured under ConPTY across a boot, a 46 s turn, an escape cancel and a
//! plan-approval wait, zero sequences — so on that version the progress
//! scanner reads `None` forever and the status machine has no way back out
//! of `running`.
//!
//! The window title still carries the state. Claude Code prefixes it with a
//! spinner glyph while a turn runs and with the idle glyph while it is
//! parked:
//!
//! | moment                     | title                        |
//! |----------------------------|------------------------------|
//! | startup, parked            | `ESC ] 0 ; ✳ Claude Code ST` |
//! | turn running               | `ESC ] 0 ; ◐ <summary> ST`   |
//! | turn ends, or is cancelled | `ESC ] 0 ; ✳ <summary> ST`   |
//!
//! The catch, and the reason this is not wired in as a `Progress` event: the
//! title also goes idle while a permission prompt waits, where the progress
//! state used to stay busy. So a title idle edge may only *end* a turn the
//! status machine currently calls `running` — never one already sitting on
//! `needs_feedback` — and it may never start one. See
//! `status::HookEvent::TitleIdle`.
//!
//! Two ways this goes quiet rather than wrong: a CLI that prefixes no glyph
//! (Claude Code's own `showStatusInTerminalTab` drops it) simply never reads
//! idle, and neither does one that titles itself something else entirely.

use super::{BEL, ESC};

/// The glyph Claude Code prefixes its title with while parked at the input
/// box. Stable across 2.1.x; `conpty_progress_probe` is what catches a
/// change.
const IDLE_GLYPH: char = '✳';

/// `<0|2>;` plus the longest UTF-8 glyph. Everything past that is the title
/// text, which this scanner has no use for.
const MAX_OSC: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Ground,
    Esc,
    /// Inside an OSC payload. `poisoned` sequences are consumed to their
    /// terminator and discarded.
    Osc {
        poisoned: bool,
    },
    /// Saw ESC inside an OSC: `ESC \` terminates (ST), anything else aborts.
    OscEsc {
        poisoned: bool,
    },
}

/// Tracks the child's advertised title glyph across chunk boundaries.
#[derive(Debug)]
pub struct TitleScanner {
    state: State,
    buf: Vec<u8>,
    /// Whether the last title the child set carried the idle glyph; `None`
    /// until it sets one.
    idle: Option<bool>,
}

impl Default for TitleScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl TitleScanner {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            buf: Vec::new(),
            idle: None,
        }
    }

    /// Whether the child's current title reads idle, or `None` if it never
    /// set one.
    pub fn idle(&self) -> Option<bool> {
        self.idle
    }

    /// Scan a chunk of child output. Sequences split across chunks are fine.
    /// Returns `Some(idle)` only when the reading *changed*, so callers see
    /// edges — the same contract as `ProgressScanner::feed`.
    pub fn feed(&mut self, data: &[u8]) -> Option<bool> {
        let before = self.idle;
        for &b in data {
            self.step(b);
        }
        match self.idle {
            after if after != before => after,
            _ => None,
        }
    }

    fn step(&mut self, b: u8) {
        match self.state {
            State::Ground => {
                if b == ESC {
                    self.state = State::Esc;
                }
            }
            State::Esc => {
                if b == b']' {
                    self.buf.clear();
                    self.state = State::Osc { poisoned: false };
                } else {
                    self.state = if b == ESC { State::Esc } else { State::Ground };
                }
            }
            State::Osc { poisoned } => match b {
                BEL => {
                    if !poisoned {
                        self.dispatch();
                    }
                    self.buf.clear();
                    self.state = State::Ground;
                }
                ESC => self.state = State::OscEsc { poisoned },
                _ => {
                    if !poisoned {
                        // Past the glyph the rest of the title is noise, so
                        // stop accumulating — but keep dispatching, unlike a
                        // payload whose prefix rules it out altogether.
                        if !prefix_possible(&self.buf) {
                            self.buf.clear();
                            self.state = State::Osc { poisoned: true };
                        } else if self.buf.len() < MAX_OSC {
                            self.buf.push(b);
                        }
                    }
                }
            },
            State::OscEsc { poisoned } => {
                if b == b'\\' {
                    if !poisoned {
                        self.dispatch();
                    }
                    self.buf.clear();
                    self.state = State::Ground;
                } else {
                    // Aborted mid-OSC; ESC ESC restarts the escape.
                    self.buf.clear();
                    self.state = if b == ESC { State::Esc } else { State::Ground };
                }
            }
        }
    }

    fn dispatch(&mut self) {
        let payload = std::mem::take(&mut self.buf);
        // OSC 0 sets icon name and title, OSC 2 the title alone; either one
        // carries the glyph.
        let Some(rest) = payload
            .strip_prefix(b"0;")
            .or_else(|| payload.strip_prefix(b"2;"))
        else {
            return;
        };
        // An empty title says nothing about the turn — leave the reading
        // alone rather than guess at it.
        let Some(first) = String::from_utf8_lossy(rest).chars().next() else {
            return;
        };
        self.idle = Some(first == IDLE_GLYPH);
    }
}

/// Could `buf` still grow into a `0;…` or `2;…` payload?
fn prefix_possible(buf: &[u8]) -> bool {
    const PREFIXES: [&[u8]; 2] = [b"0;", b"2;"];
    PREFIXES.iter().any(|prefix| {
        let n = buf.len().min(prefix.len());
        buf[..n] == prefix[..n]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(chunks: &[&[u8]]) -> Vec<Option<bool>> {
        let mut s = TitleScanner::new();
        chunks.iter().map(|c| s.feed(c)).collect()
    }

    /// The real Claude Code turn, as captured under ConPTY: parked, working,
    /// parked again. Repeats of a reading report nothing.
    #[test]
    fn reports_only_edges() {
        assert_eq!(
            scan(&[
                "\x1b]0;\u{2733} Claude Code\x1b\\".as_bytes(),
                "\x1b]0;\u{25d0} The number seven\x1b\\".as_bytes(),
                "\x1b]0;\u{25d1} The number seven\x1b\\".as_bytes(),
                "\x1b]0;\u{2733} The number seven\x1b\\".as_bytes(),
            ]),
            vec![Some(true), Some(false), None, Some(true)]
        );
    }

    /// A title split across reads is one title: the pump coalesces on a byte
    /// budget, not on sequence boundaries.
    #[test]
    fn a_split_sequence_is_still_read() {
        assert_eq!(
            scan(&[b"\x1b]0;\xe2", b"\x9c\xb3 Claude Code\x07"]),
            vec![None, Some(true)]
        );
    }

    /// A CLI that prefixes no glyph reads busy forever, which costs nothing:
    /// only the idle edge is ever acted on.
    #[test]
    fn a_title_without_the_glyph_never_reads_idle() {
        let mut s = TitleScanner::new();
        s.feed(b"\x1b]0;codex\x07");
        assert_eq!(s.idle(), Some(false));
    }

    /// The scanner shares its stream with every other OSC there is — notably
    /// OSC 9;4 itself, whose payload also starts with a digit.
    #[test]
    fn other_oscs_are_ignored() {
        let mut s = TitleScanner::new();
        s.feed("\x1b]0;\u{2733} Claude Code\x07".as_bytes());
        assert_eq!(
            s.feed(b"\x1b]9;4;3;\x07\x1b]8;;https://example.com\x1b\\\x1b]777;x\x07"),
            None,
            "a non-title OSC must not move the reading"
        );
        assert_eq!(s.idle(), Some(true));
    }

    /// An empty title is not news either way.
    #[test]
    fn an_empty_title_leaves_the_reading_alone() {
        let mut s = TitleScanner::new();
        s.feed("\x1b]0;\u{2733} Claude Code\x07".as_bytes());
        assert_eq!(s.feed(b"\x1b]0;\x07"), None);
        assert_eq!(s.idle(), Some(true));
    }
}
