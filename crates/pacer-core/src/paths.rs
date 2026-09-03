use crate::env;
use std::path::{Path, PathBuf};

/// Where the runtime dir lands when neither override nor `XDG_RUNTIME_DIR`
/// is set. World-writable, so the per-uid subdir is what carries mode 0700.
#[cfg(unix)]
const FALLBACK_RUNTIME_ROOT: &str = "/tmp";

/// Runtime dir holding the socket + pidfile. Mode 0700 — this is the auth
/// boundary, same model as tmux. `PACER_RUNTIME_DIR` overrides (tests,
/// parallel instances).
pub fn runtime_dir() -> PathBuf {
    if let Some(dir) = env::non_empty(env::RUNTIME_DIR) {
        return PathBuf::from(dir);
    }
    if let Some(dir) = env::non_empty("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("pacer");
    }
    default_runtime_dir()
}

#[cfg(unix)]
fn default_runtime_dir() -> PathBuf {
    let uid = libc_geteuid();
    Path::new(FALLBACK_RUNTIME_ROOT).join(format!("pacer-{uid}"))
}

/// Windows has no shared `/tmp` and no uid to key one off: `%TEMP%` is
/// already per-user (`…\AppData\Local\Temp`), closed to other
/// unprivileged users by the profile's inherited ACL, so the subdir needs no
/// uid suffix and carries no explicit mode.
#[cfg(windows)]
fn default_runtime_dir() -> PathBuf {
    std::env::temp_dir().join("pacer")
}

// Avoid a libc dependency in this dep-light crate for one call.
#[cfg(unix)]
fn libc_geteuid() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() }
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("daemon.sock")
}

/// The ENDPOINT FILE: where the Windows transport records the loopback port
/// and the bearer token a client presents (see `pacer_core::transport`).
/// Unused on Unix, where the socket path carries everything.
pub fn endpoint_path() -> PathBuf {
    runtime_dir().join("daemon.endpoint")
}

pub fn pidfile_path() -> PathBuf {
    runtime_dir().join("daemon.pid")
}

/// Fingerprint of the binary the running daemon was launched from, written
/// by the daemon at startup. Installers compare it against the binary they
/// just installed to tell an up-to-date daemon from a stale one.
pub fn buildstamp_path() -> PathBuf {
    runtime_dir().join("daemon.build")
}

/// The platform's per-user dirs for this app (`~/Library/Application
/// Support/dev.pacer` on macOS, `~/.local/share/pacer` on Linux,
/// `%APPDATA%\pacer` on Windows).
///
/// The organization is deliberately empty. `directories` builds the Windows
/// path as `organization\application` and the macOS bundle id by joining the
/// non-empty parts with dots, so naming both after the project would spell
/// `pacer\pacer` and `dev.pacer.pacer`. Linux ignores both fields anyway.
fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("dev", "", "pacer")
}

pub fn data_dir() -> PathBuf {
    if let Some(dir) = env::non_empty(env::DATA_DIR) {
        return PathBuf::from(dir);
    }
    project_dirs()
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| env::home_dir().unwrap_or_default().join(".pacer"))
}

pub fn db_path() -> PathBuf {
    data_dir().join("pacer.db")
}

/// User settings file (JSON). Lives beside the DB so `PACER_DATA_DIR`
/// isolates it for tests and parallel instances too.
pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn log_dir() -> PathBuf {
    // Tests and parallel instances override the data dir; keep their logs
    // beside their data instead of the real user's state dir.
    if env::non_empty(env::DATA_DIR).is_some() {
        return data_dir().join("state");
    }
    project_dirs()
        .map(|d| {
            d.state_dir()
                .map(|s| s.to_path_buf())
                .unwrap_or_else(|| d.data_dir().join("state"))
        })
        .unwrap_or_else(|| data_dir().join("state"))
}

/// Canonicalize a path for containment tests and for handing to other
/// programs, falling back to the raw path when it doesn't resolve (a deleted
/// checkout, a dir not created yet). macOS symlinks (`/tmp` →
/// `/private/tmp`) otherwise break [`contains`].
///
/// On Windows this also *undoes* half of what `canonicalize` does: it hands
/// back `C:\repo`, not the `\\?\C:\repo` verbatim form. That prefix is only
/// meaningful to the Win32 file APIs — `git` rejects it outright (`could not
/// create leading directories of '//?/C:/…'`), and it does not compare equal
/// to the same path written the ordinary way, which is how a canonical path
/// and a raw one silently stop matching. A path that leaves this function is
/// a path every consumer can take.
pub fn canonical_or_raw(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(canonical) => strip_verbatim_prefix(canonical),
        Err(_) => path.to_path_buf(),
    }
}

#[cfg(unix)]
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    path
}

/// `\\?\C:\x` → `C:\x`, `\\?\UNC\srv\share\x` → `\\srv\share\x`. Anything
/// else (a device path, a raw volume GUID) is left exactly as it came: those
/// have no ordinary spelling, and inventing one would be worse than the
/// verbatim form.
#[cfg(windows)]
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    use std::path::{Component, Prefix};
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return path;
    };
    let text = path.to_string_lossy();
    let stripped = match prefix.kind() {
        Prefix::VerbatimDisk(_) => text.strip_prefix(r"\\?\").map(str::to_string),
        Prefix::VerbatimUNC(..) => text
            .strip_prefix(r"\\?\UNC\")
            .map(|rest| format!(r"\\{rest}")),
        _ => None,
    };
    stripped.map(PathBuf::from).unwrap_or(path)
}

/// Is `inner` the same path as `outer`, or inside it?
///
/// Component-wise like `Path::starts_with`, so `/a/bc` is not inside `/a/b`,
/// and case-insensitively on Windows, where `d:\repo` and `D:\Repo` are one
/// directory — an agent's reported cwd and a stored WORKTREE path come from
/// different programs and need not agree on case.
///
/// Both sides must have been through [`canonical_or_raw`], or neither: half a
/// comparison is the failure this pair exists to prevent.
pub fn contains(outer: &Path, inner: &Path) -> bool {
    #[cfg(unix)]
    {
        inner.starts_with(outer)
    }
    #[cfg(windows)]
    {
        let mut outer = outer.components();
        let mut inner = inner.components();
        loop {
            match (outer.next(), inner.next()) {
                (None, _) => return true,
                (Some(_), None) => return false,
                (Some(a), Some(b)) => {
                    // ASCII is the whole alphabet in play here: drive letters,
                    // and path segments a full Unicode case fold would only
                    // differ on for names no repo tool produces.
                    if !a
                        .as_os_str()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
                    {
                        return false;
                    }
                }
            }
        }
    }
}

pub fn daemon_log_path() -> PathBuf {
    log_dir().join("daemon.log")
}

pub fn tui_log_path() -> PathBuf {
    log_dir().join("tui.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The containment rule every cwd → WORKTREE match rests on: a whole
    /// component, never a string prefix.
    #[test]
    fn containment_is_component_wise_and_reflexive() {
        let root = std::env::temp_dir().join("pacer-contains");
        assert!(contains(&root, &root), "a path contains itself");
        assert!(contains(&root, &root.join("a").join("b")));
        assert!(!contains(&root.join("a"), &root));
        // `…/ab` is not inside `…/a`, however the strings compare.
        assert!(!contains(&root.join("a"), &root.join("ab")));
    }

    /// Windows path text varies with whoever produced it — a hook payload's
    /// cwd, a stored WORKTREE row, `git rev-parse`. Case must not decide
    /// whether a session gets re-homed.
    #[cfg(windows)]
    #[test]
    fn containment_ignores_case_on_windows() {
        assert!(contains(
            Path::new(r"D:\web-projects\pacer"),
            Path::new(r"d:\WEB-projects\Pacer\crates")
        ));
        assert!(!contains(
            Path::new(r"D:\web-projects\pacer"),
            Path::new(r"D:\web-projects\pacer-other")
        ));
    }

    /// The verbatim prefix is what makes a canonical path stop being
    /// interchangeable with the same path written normally — and git refuses
    /// it outright. It must never leave `canonical_or_raw`.
    #[cfg(windows)]
    #[test]
    fn canonicalizing_yields_an_ordinary_windows_path() {
        let tmp = std::env::temp_dir();
        let canonical = canonical_or_raw(&tmp);
        assert!(
            !canonical.to_string_lossy().starts_with(r"\\?\"),
            "the verbatim prefix escaped: {}",
            canonical.display()
        );
        assert!(canonical.is_absolute());
        // A path that does not resolve comes back untouched.
        let absent = tmp.join("pacer-no-such-dir-9e3a1");
        assert_eq!(canonical_or_raw(&absent), absent);
    }

    #[cfg(windows)]
    #[test]
    fn only_the_spellings_with_an_ordinary_form_are_stripped() {
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\C:\repo\x")),
            PathBuf::from(r"C:\repo\x")
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\UNC\srv\share\x")),
            PathBuf::from(r"\\srv\share\x")
        );
        // A volume GUID path has no ordinary spelling; leave it alone.
        let guid = PathBuf::from(r"\\?\Volume{00000000-0000-0000-0000-000000000000}\x");
        assert_eq!(strip_verbatim_prefix(guid.clone()), guid);
        // An ordinary path is not a verbatim one.
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"C:\repo")),
            PathBuf::from(r"C:\repo")
        );
    }

    /// Restores an env var to what it was when the guard was made, so a
    /// failed assertion can't leak an override into the next test.
    struct EnvRestore(&'static str, Option<std::ffi::OsString>);
    impl EnvRestore {
        fn new(var: &'static str) -> Self {
            Self(var, std::env::var_os(var))
        }
    }
    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match &self.1 {
                Some(v) => std::env::set_var(self.0, v),
                None => std::env::remove_var(self.0),
            }
        }
    }

    // One test owns both override vars: env is process-global, and splitting
    // this into several `#[test]`s would race them across threads.
    #[test]
    fn overrides_win_only_when_non_empty() {
        let _restore = (
            EnvRestore::new(crate::env::RUNTIME_DIR),
            EnvRestore::new(crate::env::DATA_DIR),
        );
        let runtime = std::env::temp_dir().join(format!("pacer-paths-rt-{}", std::process::id()));
        let data = std::env::temp_dir().join(format!("pacer-paths-data-{}", std::process::id()));

        std::env::set_var(crate::env::RUNTIME_DIR, &runtime);
        std::env::set_var(crate::env::DATA_DIR, &data);
        assert_eq!(runtime_dir(), runtime);
        assert_eq!(socket_path(), runtime.join("daemon.sock"));
        assert_eq!(pidfile_path(), runtime.join("daemon.pid"));
        assert_eq!(buildstamp_path(), runtime.join("daemon.build"));
        assert_eq!(endpoint_path(), runtime.join("daemon.endpoint"));
        assert_eq!(data_dir(), data);
        assert_eq!(db_path(), data.join("pacer.db"));
        assert_eq!(config_path(), data.join("config.json"));
        // Logs follow the data override so isolated instances keep their
        // logs beside their state.
        assert_eq!(log_dir(), data.join("state"));
        assert_eq!(daemon_log_path(), data.join("state").join("daemon.log"));
        assert_eq!(tui_log_path(), data.join("state").join("tui.log"));

        std::env::set_var(crate::env::RUNTIME_DIR, "");
        std::env::set_var(crate::env::DATA_DIR, "");
        assert_ne!(runtime_dir(), runtime, "empty override must fall through");
        assert_ne!(data_dir(), data, "empty override must fall through");
        assert_ne!(log_dir(), data.join("state"));
        assert!(
            runtime_dir().is_absolute() && data_dir().is_absolute(),
            "defaults are absolute: {} / {}",
            runtime_dir().display(),
            data_dir().display()
        );
    }
}
