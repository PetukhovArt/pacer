//! Raw PTY capture, for debugging rendering artifacts.
//!
//! `NEBULA_PTY_CAPTURE=<dir>` makes every PTY SESSION append the exact bytes
//! its child wrote to `<dir>/<id>.raw`, plus a `<id>.meta` jsonl of the
//! resizes interleaved by byte offset. That pair is enough to replay a
//! session offline (`cargo run -p nebula-tui --example replay_capture`) and
//! settle whether a mangled screen came in mangled or was mangled here.
//!
//! Off unless the env var is set: `open` returns `None` and every call is a
//! no-op, so a normal run pays one `env::var` per session.

use nebula_core::protocol::SessionRef;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

pub const CAPTURE_DIR_ENV: &str = "NEBULA_PTY_CAPTURE";

pub struct Capture {
    raw: File,
    meta: File,
    /// Bytes written to `raw` so far — the timeline resizes are stamped on.
    offset: u64,
}

impl Capture {
    /// A capture for this session, or `None` when capture is off (or the
    /// directory can't be written — debugging aid, never a spawn failure).
    pub fn open(sref: &SessionRef) -> Option<Self> {
        let dir = PathBuf::from(std::env::var_os(CAPTURE_DIR_ENV)?);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %e, dir = ?dir, "pty capture dir");
            return None;
        }
        let stem = match sref {
            SessionRef::Agent(id) => format!("agent-{id}"),
            SessionRef::Terminal(id) => format!("terminal-{id}"),
        };
        let open = |ext: &str| File::create(dir.join(format!("{stem}.{ext}")));
        match (open("raw"), open("meta")) {
            (Ok(raw), Ok(meta)) => {
                tracing::info!(dir = ?dir, stem, "capturing raw pty output");
                Some(Self {
                    raw,
                    meta,
                    offset: 0,
                })
            }
            (raw, meta) => {
                let e = raw.err().or(meta.err());
                tracing::warn!(error = ?e, dir = ?dir, "pty capture files");
                None
            }
        }
    }

    pub fn output(&mut self, bytes: &[u8]) {
        if self.raw.write_all(bytes).is_ok() {
            self.offset += bytes.len() as u64;
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let _ = writeln!(
            self.meta,
            r#"{{"at":{},"cols":{cols},"rows":{rows}}}"#,
            self.offset
        );
    }
}

/// `Option<Capture>` reads better as a verb at the call sites than as a match.
pub trait CaptureExt {
    fn output(&mut self, bytes: &[u8]);
    fn resize(&mut self, cols: u16, rows: u16);
}

impl CaptureExt for Option<Capture> {
    fn output(&mut self, bytes: &[u8]) {
        if let Some(c) = self {
            c.output(bytes);
        }
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        if let Some(c) = self {
            c.resize(cols, rows);
        }
    }
}
