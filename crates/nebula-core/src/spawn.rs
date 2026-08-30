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

/// Hand `url` to the desktop's default browser: `open` on macOS, `xdg-open`
/// on Linux, `cmd /c start` on Windows. Returns whether the opener reported
/// success (it hands off and exits; nothing waits on the browser).
///
/// Callers own their scheme allowlists and test shortcuts — a `cfg!(test)`
/// here would be false in every crate that depends on this one.
pub fn open_in_browser(url: &str) -> bool {
    use std::process::{Command, Stdio};
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // `start` is a cmd builtin (ShellExecute under the hood), so the URL
        // crosses cmd's own parser. std quotes an argv item only when it has
        // whitespace, and an unquoted `&` in a query string would split the
        // command — so the URL is quoted by hand and passed raw. Embedded
        // `"` are stripped, not escaped: cmd has no escape `start` survives,
        // and no http(s) URL needs one. The empty quoted arg is `start`'s
        // window-title slot.
        let quoted = format!("\"{}\"", url.replace('"', ""));
        Command::new("cmd.exe")
            .args(["/c", "start", ""])
            .raw_arg(quoted)
            .no_window()
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
    #[cfg(unix)]
    {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        Command::new(opener)
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
}
