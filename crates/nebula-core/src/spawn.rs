//! One knob for helper subprocesses: don't flash a console window.
//!
//! On Windows a console child spawned from a process without a console (the
//! DETACHED_PROCESS daemon, or the TUI once its console is in raw mode's
//! alternate screen) allocates a brand-new visible console — every git poll
//! and `gh` call flashed a window while the user browsed worktrees.
//! `CREATE_NO_WINDOW` runs the child with a console but no window, which is
//! exactly what a captured-output helper wants. On Unix this is a no-op.
//!
//! Not for every spawn: `creation_flags` *replaces* the flag word, so a site
//! that needs other creation flags (the daemon auto-spawn's
//! `DETACHED_PROCESS`, see `nebula-tui`'s `ipc::detach`) must keep setting
//! its own, and interactive handoffs (`nebula ssh`, ttyd) must keep the
//! console they inherit.

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `cmd.no_window()` — chainable, platform-free at the call site.
pub trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

impl NoWindow for std::process::Command {
    #[cfg(windows)]
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW)
    }
    #[cfg(unix)]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

impl NoWindow for tokio::process::Command {
    #[cfg(windows)]
    fn no_window(&mut self) -> &mut Self {
        self.creation_flags(CREATE_NO_WINDOW)
    }
    #[cfg(unix)]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}
