//! DSR cursor-position queries (`CSI 6 n`) in a PTY's output stream.
//!
//! On Windows the ConPTY host opens with `PSEUDOCONSOLE_INHERIT_CURSOR`
//! (hardcoded in `portable-pty`), sends `ESC[6n` to whatever reads the
//! master, and **blocks the child's console connection until the reply
//! arrives** — the child never executes, which reads as "ConPTY is broken
//! on this machine". Pacer is that reader, so pacer must answer.
//!
//! Only the plain `CSI 6 n` form is matched (that is what the host sends);
//! `CSI ? 6 n` (DECXCPR) and other DSR kinds are left alone. Callers are
//! Windows-only today: on Unix nothing sends this on the master side, and
//! answering an application's own query with a made-up position would be
//! worse than the status quo of not answering at all. On Windows the
//! application's in-band queries never reach the master — conhost answers
//! them itself from its own screen state — so every `CSI 6 n` seen here is
//! the host's, and the truthful reply for a freshly spawned screen is
//! row 1, column 1.

/// Detects `CSI 6 n` across chunk boundaries; the state machine persists
/// between [`DsrScanner::feed`] calls, mirroring the daemon's other output
/// scanners.
#[derive(Debug, Default)]
pub struct DsrScanner {
    state: State,
}

#[derive(Debug, Default, Clone, Copy)]
enum State {
    #[default]
    Ground,
    Esc,
    /// Inside a CSI: `seen_six` is true only while the params so far are
    /// exactly `6`; any other param byte or an intermediate poisons it.
    Csi {
        seen_six: bool,
        poisoned: bool,
    },
}

/// The reply to each query: cursor at row 1, column 1.
pub const DSR_REPLY: &[u8] = b"\x1b[1;1R";

impl DsrScanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Scan a chunk of output; returns how many `CSI 6 n` queries completed
    /// in it. The caller writes [`DSR_REPLY`] once per query.
    pub fn feed(&mut self, data: &[u8]) -> usize {
        let mut hits = 0;
        for &b in data {
            self.step(b, &mut hits);
        }
        hits
    }

    fn step(&mut self, b: u8, hits: &mut usize) {
        const ESC: u8 = 0x1b;
        match self.state {
            State::Ground => {
                if b == ESC {
                    self.state = State::Esc;
                }
            }
            State::Esc => {
                self.state = match b {
                    b'[' => State::Csi {
                        seen_six: false,
                        poisoned: false,
                    },
                    ESC => State::Esc,
                    _ => State::Ground,
                };
            }
            State::Csi { seen_six, poisoned } => match b {
                b'6' if !poisoned && !seen_six => {
                    self.state = State::Csi {
                        seen_six: true,
                        poisoned: false,
                    };
                }
                // Any further param or intermediate byte: not a bare `6`.
                0x20..=0x3F => {
                    self.state = State::Csi {
                        seen_six,
                        poisoned: true,
                    };
                }
                0x40..=0x7E => {
                    if b == b'n' && seen_six && !poisoned {
                        *hits += 1;
                    }
                    self.state = State::Ground;
                }
                // Cancelled / malformed sequence.
                _ => self.state = State::Ground,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conpty_handshake_is_one_hit() {
        let mut s = DsrScanner::new();
        assert_eq!(s.feed(b"\x1b[?9001h\x1b[?1004h\x1b[6n"), 1);
    }

    #[test]
    fn split_across_chunks() {
        let mut s = DsrScanner::new();
        assert_eq!(s.feed(b"hello \x1b"), 0);
        assert_eq!(s.feed(b"["), 0);
        assert_eq!(s.feed(b"6"), 0);
        assert_eq!(s.feed(b"n world"), 1);
    }

    #[test]
    fn other_dsr_kinds_and_lookalikes_ignored() {
        let mut s = DsrScanner::new();
        // DECXCPR, status report, cursor moves, params around the 6.
        assert_eq!(s.feed(b"\x1b[?6n\x1b[5n\x1b[6;1H\x1b[16n\x1b[6;6n"), 0);
    }

    #[test]
    fn multiple_queries_count() {
        let mut s = DsrScanner::new();
        assert_eq!(s.feed(b"\x1b[6n\x1b[6n"), 2);
    }
}
