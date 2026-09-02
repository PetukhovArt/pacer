//! Daemon lifecycle: pidfile + advisory-lock liveness, socket path hygiene,
//! auto-spawn from the client side.

use anyhow::{Context, Result};
use pacer_core::paths;
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
use std::path::Path;

/// Guard holding the exclusive pidfile flock for the daemon's lifetime.
/// Lock possession — not file existence — is the liveness test.
pub struct PidfileLock {
    file: std::fs::File,
}

impl PidfileLock {
    /// Try to acquire the daemon lock. Returns None when another live daemon
    /// holds it.
    pub fn try_acquire() -> Result<Option<Self>> {
        ensure_runtime_dir()?;
        let path = paths::pidfile_path();
        // No truncate: the flock decides ownership, content is informational.
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("open pidfile {}", path.display()))?;
        if !try_lock_exclusive(&file) {
            return Ok(None);
        }
        // Informational only; liveness is the lock.
        let _ = fs::write(&path, format!("{}\n", std::process::id()));
        Ok(Some(Self { file }))
    }

    pub fn is_daemon_alive() -> bool {
        match Self::try_acquire() {
            // We got the lock: nobody holds it. Release immediately by drop.
            Ok(Some(_guard)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }
}

impl Drop for PidfileLock {
    fn drop(&mut self) {
        // The lock is released automatically when the handle closes on both
        // platforms; naming the file here keeps that dependency explicit.
        let _ = &self.file;
    }
}

/// Take the daemon lock on an open pidfile without blocking. True when this
/// process now holds it.
///
/// Both platforms lock the *open handle*, not the path, so the lock dies with
/// the process however it dies — that is what makes lock possession, rather
/// than file existence, the liveness test.
#[cfg(unix)]
pub(crate) fn try_lock_exclusive(file: &std::fs::File) -> bool {
    // Tiny extern shim, same dep-light idiom as pacer_core::paths.
    extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) == 0 }
}

/// The byte the PIDFILE LOCK is taken on: 1 GiB in, so it is past any pidfile
/// content and the file stays readable while the lock is held. Both the
/// daemon and the client lock the *same* byte, or they would not contend.
#[cfg(windows)]
pub const LOCK_OFFSET: u32 = 0x4000_0000;

/// `LockFileEx` is the Windows equivalent of `flock(LOCK_EX|LOCK_NB)`:
/// `LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY` returns 0 rather
/// than waiting when another process holds the range.
///
/// The range is one byte at [`LOCK_OFFSET`], far past any pidfile content,
/// and that offset is the whole point. Windows file locks are **mandatory**,
/// not advisory like `flock`: a locked byte 0 would make the pidfile
/// unreadable to every other process, so `read_to_string` on it fails with
/// ERROR_LOCK_VIOLATION — breaking exactly the two readers that exist,
/// `pacer kill`'s pid lookup and the VERSION SKEW message's daemon path.
/// Locking past the end of the file is legal and leaves the content readable
/// while still refusing a second holder.
#[cfg(windows)]
pub(crate) fn try_lock_exclusive(file: &std::fs::File) -> bool {
    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: *mut core::ffi::c_void,
    }
    extern "system" {
        fn LockFileEx(
            handle: *mut core::ffi::c_void,
            flags: u32,
            reserved: u32,
            bytes_low: u32,
            bytes_high: u32,
            overlapped: *mut Overlapped,
        ) -> i32;
    }
    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x1;
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x2;
    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: LOCK_OFFSET,
        offset_high: 0,
        event: std::ptr::null_mut(),
    };
    unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        ) != 0
    }
}

/// Record this process's binary fingerprint so installers can tell whether
/// the running daemon is already on the build they just installed. Called by
/// the daemon at startup; best-effort (staleness checks treat a missing
/// stamp as "unknown build", which reads as stale).
pub fn write_buildstamp() {
    if let Some(stamp) = exe_buildstamp() {
        let _ = fs::write(paths::buildstamp_path(), stamp);
    }
}

/// True when a live daemon is running different code than this binary — or
/// predates buildstamps entirely, so its build is unknown.
pub fn daemon_is_stale() -> bool {
    if !PidfileLock::is_daemon_alive() {
        return false;
    }
    match (
        fs::read_to_string(paths::buildstamp_path()).ok(),
        exe_buildstamp(),
    ) {
        (Some(recorded), Some(current)) => recorded.trim() != current,
        _ => true,
    }
}

/// Content fingerprint of this process's executable.
fn exe_buildstamp() -> Option<String> {
    fingerprint_file(&std::env::current_exe().ok()?)
}

/// FNV-style multiply-xor over 8-byte words, then the length — an identity
/// check, not security, and word-wide because the daemon hashes its own
/// ~30MB debug binary at startup under the e2e tests.
fn fingerprint_file(path: &Path) -> Option<String> {
    const PRIME: u64 = 0x100_0000_01b3;
    let bytes = fs::read(path).ok()?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut words = bytes.chunks_exact(8);
    for word in &mut words {
        hash = (hash ^ u64::from_le_bytes(word.try_into().unwrap())).wrapping_mul(PRIME);
    }
    let mut tail = [0u8; 8];
    tail[..words.remainder().len()].copy_from_slice(words.remainder());
    hash = (hash ^ u64::from_le_bytes(tail)).wrapping_mul(PRIME);
    hash = (hash ^ bytes.len() as u64).wrapping_mul(PRIME);
    Some(format!("{hash:016x}"))
}

/// Create the runtime dir with 0700 perms — this is the auth boundary.
#[cfg(unix)]
pub fn ensure_runtime_dir() -> Result<()> {
    let dir = paths::runtime_dir();
    if !dir.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .with_context(|| format!("create runtime dir {}", dir.display()))?;
    } else {
        let meta = fs::metadata(&dir)?;
        let mode = meta.permissions().mode() & 0o777;
        if mode != 0o700 {
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// Create the runtime dir. No explicit ACL: the Windows default RUNTIME DIR
/// sits under `%TEMP%` inside the user's profile, which already inherits an
/// ACL granting only that user and the administrators. Writing a bespoke
/// DACL here would be strictly weaker than what it inherits and would need
/// `windows-sys` for no gain — the 0700 the Unix branch sets is the same
/// boundary, expressed the way that platform expresses it.
#[cfg(windows)]
pub fn ensure_runtime_dir() -> Result<()> {
    let dir = paths::runtime_dir();
    fs::create_dir_all(&dir).with_context(|| format!("create runtime dir {}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The PIDFILE LOCK is what refuses a second DAEMON, and its two
    /// implementations (`flock` / `LockFileEx`) are the only place the
    /// platforms differ — so the contract is asserted directly on an open
    /// handle rather than through `try_acquire`, which would need the
    /// process-global RUNTIME DIR override.
    #[test]
    fn a_second_holder_is_refused_and_closing_the_first_releases_the_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("daemon.pid");
        let open = || {
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .unwrap()
        };

        let held = open();
        assert!(try_lock_exclusive(&held), "an unlocked pidfile is takeable");
        assert!(
            !try_lock_exclusive(&open()),
            "a second daemon must be refused while the first holds the lock"
        );

        drop(held);
        assert!(
            try_lock_exclusive(&open()),
            "closing the holder's handle releases the lock"
        );
    }

    #[test]
    fn fingerprint_is_stable_for_identical_content() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        // Same bytes at different paths/inodes — the cp+mv install dance.
        fs::write(&a, b"identical build bytes").unwrap();
        fs::write(&b, b"identical build bytes").unwrap();
        assert_eq!(fingerprint_file(&a), fingerprint_file(&b));
    }

    #[test]
    fn different_content_gets_a_different_fingerprint() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("f");
        // Lengths off and on the 8-byte word boundary, plus zero-padding
        // ambiguity: "x" vs "x\0" must differ even though the padded tail
        // word is identical.
        let mut seen = std::collections::HashSet::new();
        for content in [&b""[..], b"x", b"x\0", b"12345678", b"123456789"] {
            fs::write(&file, content).unwrap();
            assert!(seen.insert(fingerprint_file(&file).unwrap()));
        }
    }
}
