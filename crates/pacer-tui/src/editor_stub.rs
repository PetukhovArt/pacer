//! Stand-in "editors" for the tests that exercise the EDITOR modal
//! (`vim_term`, and the overlays in `event_loop` that open one).
//!
//! Those tests are about pacer's own plumbing — does output reach the
//! parser, does a keystroke reach the child, is `Exited` stamped with the
//! right generation — so they need a child that behaves predictably, not a
//! real vim. A shell one-liner is the natural way to write one, and the shell
//! is the one part that cannot be shared: `/bin/sh` is not a path
//! `CreateProcess` can resolve (it is an MSYS mapping, not a file).
//!
//! Windows uses PowerShell rather than `cmd.exe`, deliberately. `cmd` needs
//! `/v:on` before `!var!` expands at run time rather than at parse time, and
//! its `&` separator has to survive two levels of quoting on the way through
//! `CreateProcessW` — a one-liner that silently produces nothing is the
//! failure mode, and a stub that silently produces nothing looks exactly like
//! a bug in the code under test.
//!
//! Every shape here must **exit on its own or die when killed**: a leaked
//! child holds the PTY open, the reader thread never sees EOF, and the test
//! binary hangs at exit instead of failing.

/// A program that stands in for the configured editor: it is handed
/// `+<line> <file>` and only has to *spawn*, so the tests can assert that the
/// EDITOR modal opened at all.
///
/// `cmd.exe` looks like the obvious Windows pick and is the wrong one: given
/// arguments it does not recognise it opens an *interactive* shell, so every
/// test that spawns one leaks a live child. `where.exe` takes the arguments,
/// fails, and is gone.
pub fn program() -> &'static str {
    #[cfg(unix)]
    {
        "/bin/sh"
    }
    #[cfg(windows)]
    {
        "where.exe"
    }
}

/// A child that stays up until it is killed, printing nothing.
pub fn idles() -> (String, Vec<String>) {
    #[cfg(unix)]
    {
        shell("sleep 30")
    }
    #[cfg(windows)]
    {
        shell("Start-Sleep 30")
    }
}

/// A child that prints `marker`, then stays up until it is killed.
pub fn prints_then_idles(marker: &str) -> (String, Vec<String>) {
    #[cfg(unix)]
    {
        shell(&format!("printf '{marker}'; sleep 30"))
    }
    #[cfg(windows)]
    {
        shell(&format!("Write-Host {marker}; Start-Sleep 30"))
    }
}

/// A child that reads one line and echoes it back as `GOT:<line>`, then
/// exits — so a test can assert on input arriving *and* on the exit.
pub fn echoes_one_line() -> (String, Vec<String>) {
    #[cfg(unix)]
    {
        shell("read line; printf \"GOT:$line\"")
    }
    #[cfg(windows)]
    {
        // `[Console]::In.ReadLine()` rather than `Read-Host`: `Read-Host`
        // writes its own prompt to the screen the test then asserts on.
        shell("$line = [Console]::In.ReadLine(); Write-Host \"GOT:$line\"")
    }
}

/// The shell that runs a one-liner, with the flags that make it take one.
fn shell(command: &str) -> (String, Vec<String>) {
    #[cfg(unix)]
    let (program, flags): (&str, &[&str]) = ("/bin/sh", &["-c"]);
    #[cfg(windows)]
    let (program, flags): (&str, &[&str]) = ("powershell.exe", &["-NoProfile", "-Command"]);
    let args = flags
        .iter()
        .map(|s| s.to_string())
        .chain(std::iter::once(command.to_string()))
        .collect();
    (program.to_string(), args)
}

// ---------------------------------------------------------------------------
// The "ConPTY child never runs" blocker was the `INHERIT_CURSOR` handshake:
// `portable-pty` opens the pseudo console with that flag, the host sends
// `ESC[6n` to the master reader, and the child's console connection blocks
// until someone replies. Whoever reads the master must answer — pacer now
// does (`pacer_core::dsr`), and every PTY test runs on Windows again.
// A child that *does* fail to run also never exits and cannot be reaped, so
// the failure mode of regressing this is the test binary hanging at exit,
// not a red test.
