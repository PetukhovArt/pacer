//! Raw capture of what the TUI writes to the outer terminal, the twin of
//! the daemon's `PACER_PTY_CAPTURE` (`pacer-daemon/src/pty/capture.rs`).
//!
//! `PACER_TUI_CAPTURE=<dir>` appends every byte the backend emits to
//! `<dir>/tui-<pid>.raw`. Replay it with the same tool as a PTY capture
//! (`cargo run -p pacer-tui --example replay_capture -- <file> --cols N
//! --rows N`): if the replayed grid shows a rendering artifact, we emitted
//! it; if the grid is clean, the outer terminal mangled a clean stream.
//!
//! Off unless the env var is set — one `env::var` at startup.

use std::fs::File;
use std::io::{BufWriter, Stdout, Write};

pub const CAPTURE_DIR_ENV: &str = "PACER_TUI_CAPTURE";

/// The backend's writer: buffered stdout, plus an optional capture file
/// that sees exactly the bytes stdout sees.
pub struct FrameTap {
    inner: BufWriter<Stdout>,
    tap: Option<File>,
}

impl FrameTap {
    pub fn new(inner: BufWriter<Stdout>) -> Self {
        let tap = std::env::var_os(CAPTURE_DIR_ENV).and_then(|dir| {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).ok()?;
            File::create(dir.join(format!("tui-{}.raw", std::process::id()))).ok()
        });
        Self { inner, tap }
    }
}

impl Write for FrameTap {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        if let Some(tap) = &mut self.tap {
            let _ = tap.write_all(&buf[..n]);
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
