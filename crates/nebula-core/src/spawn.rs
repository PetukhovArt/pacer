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

#[cfg(windows)]
pub use windows_resolve::{resolve_editor_program, resolve_program};

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

/// Windows-only: resolve bare program names the way a shell would, because
/// `CreateProcess` will not. Unix needs none of this — spawns there go
/// through the user's login shell (see `nebula-daemon`'s `launch`).
#[cfg(windows)]
mod windows_resolve {
    use std::path::{Path, PathBuf};

    /// Resolve a program name the way a Windows shell does: an explicit path
    /// is taken as written, a bare name is searched down `PATH`, and either
    /// may be missing the extension, which `PATHEXT` supplies.
    pub fn resolve_program(program: &str) -> Option<PathBuf> {
        let extensions = pathext();
        let path_var = std::env::var_os("PATH").unwrap_or_default();

        let named = Path::new(program);
        if named.components().count() > 1 || named.is_absolute() {
            return first_existing(named, &extensions);
        }
        std::env::split_paths(&path_var)
            .filter(|dir| !dir.as_os_str().is_empty())
            .find_map(|dir| first_existing(&dir.join(program), &extensions))
    }

    /// Resolve the configured editor — `resolve_program`, then Git for
    /// Windows' `usr\bin` as a fallback for a bare name that PATH doesn't
    /// carry. nebula's audience always has Git for Windows, and it ships
    /// `vim.exe` (and friends) in an `usr\bin` that PowerShell PATHs never
    /// expose — only `Git\cmd` is. Editor-only on purpose: agent CLIs must
    /// fail honestly rather than silently resolve into MSYS binaries.
    pub fn resolve_editor_program(name: &str) -> Option<PathBuf> {
        if let Some(found) = resolve_program(name) {
            return Some(found);
        }
        if Path::new(name).components().count() > 1 {
            return None; // An explicit path means that path, not a stand-in.
        }
        git_usr_bin(&resolve_program("git.exe")?, name, &pathext())
    }

    /// `<git install root>\usr\bin\<name>`, derived from where `git.exe`
    /// resolved: `<root>\cmd`, `<root>\bin`, or `<root>\mingw64\bin` — the
    /// three places a Git for Windows PATH entry can point.
    fn git_usr_bin(git: &Path, name: &str, extensions: &str) -> Option<PathBuf> {
        let dir = git.parent()?;
        let root = match dir.file_name()?.to_str()?.to_ascii_lowercase().as_str() {
            "cmd" => dir.parent()?,
            "bin" => {
                let up = dir.parent()?;
                match up.file_name().and_then(|n| n.to_str()) {
                    Some(n)
                        if n.eq_ignore_ascii_case("mingw64")
                            || n.eq_ignore_ascii_case("mingw32") =>
                    {
                        up.parent()?
                    }
                    _ => up,
                }
            }
            _ => return None,
        };
        first_existing(&root.join("usr").join("bin").join(name), extensions)
    }

    fn pathext() -> String {
        std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
    }

    /// `base` itself if it is a file, else `base` + each `PATHEXT` entry.
    fn first_existing(base: &Path, extensions: &str) -> Option<PathBuf> {
        if base.is_file() {
            return Some(base.to_path_buf());
        }
        extensions
            .split(';')
            .map(str::trim)
            .filter(|ext| !ext.is_empty())
            // PATHEXT entries carry the leading dot; appending rather than
            // using `set_extension` keeps `cursor-agent` from becoming
            // `cursor.exe` on a name that already contains a dot.
            .map(|ext| PathBuf::from(format!("{}{ext}", base.display())))
            .find(|candidate| candidate.is_file())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The resolved path is compared case-insensitively: PATHEXT is
        /// conventionally upper-case while the file on disk is not, and NTFS
        /// treats the two as one name. What matters is *which file*, not how
        /// it is spelled.
        fn assert_resolves(base: &Path, extensions: &str, expected: Option<&Path>) {
            let got = first_existing(base, extensions);
            let normalize = |p: &Path| p.to_string_lossy().to_lowercase();
            assert_eq!(
                got.as_deref().map(normalize),
                expected.map(normalize),
                "resolving {}",
                base.display()
            );
        }

        /// The two things a Windows shell does that `CreateProcess` will not:
        /// supply the extension, and search `PATH` for a bare name.
        #[test]
        fn a_bare_name_resolves_through_path_and_pathext() {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path();
            std::fs::write(dir.join("stub-tool.cmd"), b"@echo off\n").unwrap();
            std::fs::write(dir.join("plain-tool"), b"").unwrap();

            let extensions = ".COM;.EXE;.BAT;.CMD";
            assert_resolves(
                &dir.join("stub-tool"),
                extensions,
                Some(&dir.join("stub-tool.cmd")),
            );
            assert_resolves(
                &dir.join("plain-tool"),
                extensions,
                Some(&dir.join("plain-tool")),
            );
            assert_resolves(&dir.join("absent"), extensions, None);
        }

        /// A name that already contains a dot must gain the extension, not
        /// have its own replaced — `cursor-agent` is fine, but a future
        /// `foo.js` shim would break under `set_extension`.
        #[test]
        fn pathext_appends_and_never_replaces_an_existing_dot() {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path();
            std::fs::write(dir.join("foo.js.cmd"), b"").unwrap();
            assert_resolves(
                &dir.join("foo.js"),
                ".EXE;.CMD",
                Some(&dir.join("foo.js.cmd")),
            );
        }

        /// The whole point of the lookup: a bare name, found by walking PATH.
        #[test]
        fn path_is_walked_in_order_for_a_bare_name() {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path();
            std::fs::write(dir.join("nebula-probe-stub.exe"), b"").unwrap();
            let previous = std::env::var_os("PATH");
            std::env::set_var("PATH", dir);
            let found = resolve_program("nebula-probe-stub");
            match previous {
                Some(v) => std::env::set_var("PATH", v),
                None => std::env::remove_var("PATH"),
            }
            assert!(found.is_some(), "a bare name must be found down PATH");
        }

        /// The Git-for-Windows fallback must find `usr\bin\vim.exe` from any
        /// of the three dirs a PATH entry resolves `git.exe` in, and stay
        /// silent when the editor isn't shipped there.
        #[test]
        fn git_usr_bin_is_derived_from_each_git_location() {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            for dir in ["cmd", "bin", r"mingw64\bin", r"usr\bin"] {
                std::fs::create_dir_all(root.join(dir)).unwrap();
            }
            let vim = root.join(r"usr\bin\vim.exe");
            std::fs::write(&vim, b"").unwrap();

            let extensions = ".COM;.EXE";
            // Case-normalized like `assert_resolves`: PATHEXT supplies `.EXE`.
            let normalize = |p: &Path| p.to_string_lossy().to_lowercase();
            for git in [r"cmd\git.exe", r"bin\git.exe", r"mingw64\bin\git.exe"] {
                let got = git_usr_bin(&root.join(git), "vim", extensions);
                assert_eq!(
                    got.as_deref().map(normalize),
                    Some(normalize(&vim)),
                    "from {git}"
                );
            }
            assert_eq!(
                git_usr_bin(&root.join(r"cmd\git.exe"), "hx", extensions),
                None,
                "an editor Git doesn't ship must stay unresolved"
            );
            assert_eq!(
                git_usr_bin(Path::new(r"C:\somewhere\else\git.exe"), "vim", extensions),
                None,
                "an unrecognised git layout must not invent a root"
            );
        }
    }
}
