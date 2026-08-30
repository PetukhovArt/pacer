//! Stand-in "editors" for the tests that exercise the EDITOR modal
//! (`vim_term`, and the overlays in `event_loop` that open one).
//!
//! Those tests are about nebula's own plumbing — does output reach the
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
// Known blocker: the ConPTY child never runs on this Windows machine.
//
// `portable-pty` 0.9 opens the pseudo console fine — its handshake
// (`ESC[?9001h ESC[?1004h ESC[6n`) reaches the master reader — but the child
// spawned into it produces no output and either hangs or exits with
// STATUS_DLL_INIT_FAILED (0xC0000142). It reproduces outside nebula with
// `cmd.exe /c echo` as the child, with the sideloaded WezTerm `conpty.dll`
// both on and off `PATH`, and inside and outside the agent's sandbox.
//
// A child that never runs also never exits, so it cannot be reaped: the
// reader thread never sees EOF and the *test binary* hangs at exit instead of
// failing. That is why every test that opens a real PTY is `#[cfg(unix)]` for
// now, not merely expected to fail. Everything that does not need a live
// child — the DAEMON SOCKET, the PIDFILE LOCK, path handling, PATH/PATHEXT
// resolution, the whole TUI render grid — runs on Windows and passes.
//
// Until it is resolved, PTY SESSIONS cannot be verified end-to-end here.
