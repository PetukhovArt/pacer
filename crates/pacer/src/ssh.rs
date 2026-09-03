//! `pacer ssh HOST [PATH]`: open pacer on a remote machine.
//!
//! Runs `ssh -t HOST <cmd>` so the remote TUI renders in this terminal. The
//! remote command installs pacer via the published install script when it
//! isn't on the remote PATH, then launches it.
//!
//! The remote is POSIX either way — the script below is what a remote login
//! shell runs, and nothing about it changes with the local platform. Only
//! *how this process hands over the terminal* differs: see
//! [`hand_terminal_to_ssh`].
//!
//! Quoting: sshd hands the command string to the user's login shell, which
//! may be bash, zsh, or fish. The script below is a fixed constant with no
//! single quotes, backslashes, or newlines, wrapped once in '...'; user input
//! (install URL, start dir) is passed only as positional parameters, each
//! POSIX-single-quoted. csh/tcsh login shells are the one unsupported case.

use anyhow::{bail, Context, Result};
use std::process::Command;

/// The opening half of every remote script: leave a usable `pacer` on the
/// remote PATH, installing it first when there is none. `$1` is the install
/// URL. A macro rather than a const because `concat!` only takes literals,
/// and [`crate::tunnel`] builds a different tail onto the same head.
macro_rules! install_prelude {
    () => {
        concat!(
            // sshd hands a remote command a bare PATH — no login shell runs,
            // so nothing the user configured applies. Prepend install.sh's
            // default PACER_INSTALL_DIR, and append both Homebrew prefixes:
            // on a macOS remote that is the only place ttyd (which
            // `pacer browser` needs) or a brew-installed pacer lives.
            "export PATH=\"$HOME/.local/bin:$PATH:/opt/homebrew/bin:/usr/local/bin\"; ",
            "if ! command -v pacer >/dev/null 2>&1; then ",
            "command -v curl >/dev/null 2>&1 || { ",
            "echo \"pacer: curl is required on the remote to install pacer\" >&2; exit 127; }; ",
            "echo \"pacer not found on remote; installing...\" >&2; ",
            "curl -fsSL \"$1\" | sh || exit 1; ",
            "fi; "
        )
    };
}
pub(crate) use install_prelude;

/// Runs under `sh -c` on the remote: $1 = install URL, $2 = start dir
/// (optional; defaults to the remote $HOME).
const REMOTE_SCRIPT: &str = concat!(
    install_prelude!(),
    "cd -- \"${2:-$HOME}\" || exit 1; ",
    "exec pacer"
);

pub fn run_ssh(host: &str, path: Option<&str>) -> Result<()> {
    // Remember the destination for the TUI's `h` picker. Before the exec on
    // purpose (there is no after); a host that fails to connect still lists,
    // and `d` can drop it.
    pacer_tui::hosts::record(host, path);
    let cmd = remote_command(&crate::upgrade::install_url(), path);
    hand_terminal_to_ssh(host, &cmd)
}

/// Give `ssh` this terminal and this process's exit status.
///
/// `exec` is the exact fit: ssh replaces us, so it owns the tty outright and
/// its exit status *is* ours, with no wrapper process left to forward
/// signals through. Only returns on failure.
#[cfg(unix)]
fn hand_terminal_to_ssh(host: &str, cmd: &str) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let err = Command::new("ssh").args(["-t", "--", host, cmd]).exec();
    if err.kind() == std::io::ErrorKind::NotFound {
        bail!("ssh not found on PATH — pacer ssh requires the OpenSSH client");
    }
    Err(err).context("failed to exec ssh")
}

/// Windows has no `exec`, so this process stays alive as ssh's parent and
/// has to reproduce by hand the two things `exec` gave for free.
///
/// *The terminal*: ssh inherits the console, so the remote TUI renders here
/// as it should. But Ctrl+C at that console is delivered to every process in
/// the group — us included — and the default handler would kill this parent
/// out from under the running session. So the handler is disabled for the
/// duration: ssh in `-t` mode forwards the keystroke to the remote as input,
/// which is where it belongs, and the local process must not act on it.
///
/// *The exit status*: propagated by exiting with the child's code rather
/// than returning, so a caller cannot accidentally reframe it as success.
#[cfg(windows)]
fn hand_terminal_to_ssh(host: &str, cmd: &str) -> Result<()> {
    let mut child = match Command::new("ssh").args(["-t", "--", host, cmd]).spawn() {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            bail!("ssh not found on PATH — pacer ssh requires the OpenSSH client")
        }
        Err(e) => return Err(e).context("failed to spawn ssh"),
    };
    let _ctrl_c = IgnoreCtrlC::install();
    let status = child.wait().context("failed to wait for ssh")?;
    std::process::exit(status.code().unwrap_or(1));
}

/// Suppresses this process's own Ctrl+C handling while it is alive, and puts
/// it back on drop. `SetConsoleCtrlHandler(NULL, TRUE)` is the documented way
/// to say "ignore Ctrl+C" — it is inherited by children, but ssh installs its
/// own handler over it, so the keystroke still reaches the remote.
#[cfg(windows)]
struct IgnoreCtrlC(bool);

#[cfg(windows)]
impl IgnoreCtrlC {
    fn install() -> Self {
        Self(set_ignore_ctrl_c(true))
    }
}

#[cfg(windows)]
impl Drop for IgnoreCtrlC {
    fn drop(&mut self) {
        if self.0 {
            set_ignore_ctrl_c(false);
        }
    }
}

#[cfg(windows)]
fn set_ignore_ctrl_c(ignore: bool) -> bool {
    extern "system" {
        fn SetConsoleCtrlHandler(handler: *const core::ffi::c_void, add: i32) -> i32;
    }
    unsafe { SetConsoleCtrlHandler(std::ptr::null(), i32::from(ignore)) != 0 }
}

fn remote_command(install_url: &str, path: Option<&str>) -> String {
    let mut cmd = format!(
        "sh -c '{}' pacer-ssh {}",
        REMOTE_SCRIPT,
        shell_single_quote(install_url)
    );
    if let Some(path) = path {
        cmd.push(' ');
        cmd.push_str(&shell_single_quote(path));
    }
    cmd
}

/// POSIX-quote for a remote shell: `it's` -> `'it'\''s'`.
pub(crate) fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://example.com/install.sh";

    #[test]
    fn script_survives_single_quoting() {
        // The whole scheme rests on the script needing no escaping inside
        // '...' under any login shell.
        assert!(!REMOTE_SCRIPT.contains('\''));
        assert!(!REMOTE_SCRIPT.contains('\\'));
        assert!(!REMOTE_SCRIPT.contains('\n'));
    }

    #[test]
    fn no_path_defaults_to_remote_home() {
        let cmd = remote_command(URL, None);
        assert!(cmd.ends_with("pacer-ssh 'https://example.com/install.sh'"));
        assert!(cmd.contains("${2:-$HOME}"));
    }

    #[test]
    fn path_is_quoted() {
        let cmd = remote_command(URL, Some("/srv/my repo"));
        assert!(cmd.ends_with("'/srv/my repo'"));
    }

    #[test]
    fn path_with_single_quote_is_escaped() {
        let cmd = remote_command(URL, Some("/tmp/it's here"));
        assert!(cmd.ends_with("'/tmp/it'\\''s here'"));
    }

    #[test]
    fn single_quote_edge_cases() {
        assert_eq!(shell_single_quote(""), "''");
        assert_eq!(shell_single_quote("plain"), "'plain'");
        assert_eq!(shell_single_quote("'''"), "''\\'''\\'''\\'''");
    }
}
