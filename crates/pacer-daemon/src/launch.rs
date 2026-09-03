//! The environment a PTY SESSION's child is launched into, and how the
//! daemon finds out whether an agent CLI is installed at all.
//!
//! On Unix this is one idea: run everything through the user's **login +
//! interactive shell**, so an agent CLI sees the same `PATH` and env a
//! Terminal.app tab would (`/etc/zprofile`, `~/.zprofile`, `~/.zshrc`,
//! `path_helper`) rather than whatever the daemon inherited at boot. `exec`
//! keeps the CLI as the PTY's direct process, and DAEMON SETSID keeps the
//! interactive shell off the daemon's controlling terminal.
//!
//! Windows has no equivalent and needs none: a process started from Explorer,
//! a service or another process reads the same registry-held `PATH` as one
//! started from a shell — there is no per-shell login env to recover. So the
//! CLI is launched directly, and the only thing that has to be recovered by
//! hand is what a shell would otherwise do: resolve a bare program name
//! against `PATH` × `PATHEXT`, and hand a `.cmd` / `.bat` shim to `cmd.exe`,
//! which is the only thing that can execute one.

#[cfg(windows)]
use std::path::Path;

/// The program + args a plain TERMINAL SESSION opens on.
pub fn interactive_shell() -> (String, Vec<String>) {
    platform::interactive_shell()
}

/// Turn `program args…` into what should actually be spawned so the child
/// gets the user's real environment.
pub fn wrap_for_user_env(program: &str, args: &[String]) -> (String, Vec<String>) {
    platform::wrap_for_user_env(program, args)
}

/// Is `program` installed and runnable?
///
/// `None` means the probe itself could not answer — it timed out, or the
/// shell would not start. That is deliberately distinct from `Some(false)`:
/// callers cache a verdict but must not cache, or act on, a non-answer.
pub async fn program_is_installed(program: &str, timeout: std::time::Duration) -> Option<bool> {
    platform::program_is_installed(program, timeout).await
}

// ---------------------------------------------------------------------------
// Unix
// ---------------------------------------------------------------------------
#[cfg(unix)]
mod platform {
    /// `-l` login, `-i` interactive, `-c` take the command line: between them
    /// the child sees the profile and rc files a real terminal tab would.
    const LOGIN_SHELL_ARGS: [&str; 3] = ["-l", "-i", "-c"];

    fn user_shell() -> String {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }

    pub fn interactive_shell() -> (String, Vec<String>) {
        // `-l` makes it a login shell, matching Terminal.app: zsh then
        // sources /etc/zprofile (path_helper), ~/.zprofile, and ~/.zshrc.
        (user_shell(), vec!["-l".into()])
    }

    pub fn wrap_for_user_env(program: &str, args: &[String]) -> (String, Vec<String>) {
        login_shell_wrap(&user_shell(), program, args)
    }

    /// Wrap `program args…` in a login + interactive shell (`$SHELL -l -i -c
    /// 'exec …'`). `exec` keeps the child as the PTY's direct process, so
    /// exit codes and signals pass through.
    pub(super) fn login_shell_wrap(
        shell: &str,
        program: &str,
        args: &[String],
    ) -> (String, Vec<String>) {
        let mut cmdline = String::from("exec");
        for part in std::iter::once(program).chain(args.iter().map(String::as_str)) {
            cmdline.push_str(" '");
            cmdline.push_str(&part.replace('\'', "'\\''"));
            cmdline.push('\'');
        }
        let args = LOGIN_SHELL_ARGS
            .iter()
            .map(|s| s.to_string())
            .chain([cmdline])
            .collect();
        (shell.to_string(), args)
    }

    pub async fn program_is_installed(program: &str, timeout: std::time::Duration) -> Option<bool> {
        // Asked through the same login shell the CLI will be launched with,
        // or the answer would be about the daemon's PATH, not the user's.
        let check = format!("command -v '{program}' >/dev/null 2>&1");
        let mut probe = tokio::process::Command::new(user_shell());
        probe
            .args(LOGIN_SHELL_ARGS)
            .arg(&check)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            // A timed-out probe must die with the dropped future, not linger.
            .kill_on_drop(true);
        // Own session: the interactive shell must not reach the daemon's
        // controlling terminal (--foreground runs have one). zsh's job-control
        // init opens /dev/tty and makes itself the foreground process group,
        // SIGTTIN-stopping whatever TUI owns that terminal.
        unsafe {
            probe.pre_exec(|| match nix::unistd::setsid() {
                Ok(_) => Ok(()),
                Err(errno) => Err(std::io::Error::from_raw_os_error(errno as i32)),
            });
        }
        match tokio::time::timeout(timeout, probe.status()).await {
            Ok(Ok(status)) => Some(status.success()),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn login_shell_wrap_quotes_and_execs() {
            let (program, args) = login_shell_wrap(
                "/bin/zsh",
                "claude",
                &["--resume".to_string(), "sid-1".to_string()],
            );
            assert_eq!(program, "/bin/zsh");
            assert_eq!(
                args,
                vec!["-l", "-i", "-c", "exec 'claude' '--resume' 'sid-1'"]
            );
            // Single quotes in an arg survive the wrapping.
            let (_, args) = login_shell_wrap("/bin/zsh", "echo", &["it's".to_string()]);
            assert_eq!(args[3], r"exec 'echo' 'it'\''s'");
        }
    }
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod platform {
    use super::*;
    // The PATH × PATHEXT lookup lives in pacer-core so the TUI's editor
    // spawn can share it; this module keeps the daemon-only policy (shell
    // choice, `.cmd` shim wrapping).
    use pacer_core::spawn::resolve_program;

    pub fn interactive_shell() -> (String, Vec<String>) {
        // `SHELL` wins when it names something this OS can actually start —
        // a user who points it at `bash.exe` means it. Git Bash's own
        // `SHELL=/usr/bin/bash` is an MSYS mapping, not a path CreateProcess
        // can resolve, so it falls through here rather than failing at spawn.
        if let Some(shell) = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|s| resolve_program(&s))
        {
            return (shell.display().to_string(), Vec::new());
        }
        // PowerShell reads the user's profile on its own, which is what the
        // Unix side's `-l` was for. `pwsh` first: it is the version a user
        // who installed one chose.
        for candidate in ["pwsh.exe", "powershell.exe"] {
            if let Some(path) = resolve_program(candidate) {
                return (path.display().to_string(), Vec::new());
            }
        }
        (comspec(), Vec::new())
    }

    pub fn wrap_for_user_env(program: &str, args: &[String]) -> (String, Vec<String>) {
        let Some(resolved) = resolve_program(program) else {
            // Unresolved: hand it over untouched so the spawn fails with the
            // OS's own "not found", rather than inventing a diagnosis here.
            return (program.to_string(), args.to_vec());
        };
        if !is_batch_file(&resolved) {
            return (resolved.display().to_string(), args.to_vec());
        }
        // npm installs `codex` and `cursor-agent` as `.cmd` shims, and
        // `CreateProcess` cannot execute one — only `cmd.exe` can. `call`
        // ahead of the shim keeps `cmd /c`'s quote-stripping rule off the
        // path, which would otherwise mangle a quoted program path.
        //
        // Caveat, recorded rather than guarded: from here on the arguments
        // are parsed by `cmd.exe`, not by `CommandLineToArgvW`, so an arg
        // containing `&`, `|`, `^` or `%` is at the mercy of cmd's own
        // rules. Agent argv is model names, flags and prompts; if a prompt
        // ever comes through mangled, this is the place.
        let mut wrapped = vec!["/c".to_string(), "call".to_string()];
        wrapped.push(resolved.display().to_string());
        wrapped.extend(args.iter().cloned());
        (comspec(), wrapped)
    }

    pub async fn program_is_installed(
        program: &str,
        _timeout: std::time::Duration,
    ) -> Option<bool> {
        // No subprocess and no timeout to honour: "installed" is exactly the
        // PATH × PATHEXT lookup a shell would do, and doing it in-process
        // makes the answer immediate instead of costing a shell start — so
        // this branch never has to say "could not tell".
        Some(resolve_program(program).is_some())
    }

    fn comspec() -> String {
        std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into())
    }

    fn is_batch_file(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// A `.cmd` shim has to go through `cmd.exe`; a real executable must
        /// not, or every agent would gain a stray `cmd.exe` parent that the
        /// PTY SESSION's hangup has to travel through.
        #[test]
        fn only_batch_shims_are_handed_to_cmd_exe() {
            assert!(is_batch_file(Path::new(r"C:\npm\codex.CMD")));
            assert!(is_batch_file(Path::new(r"C:\npm\codex.bat")));
            assert!(!is_batch_file(Path::new(r"C:\bin\claude.exe")));
            assert!(!is_batch_file(Path::new(r"C:\bin\claude")));
        }
    }
}
