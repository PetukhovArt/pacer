//! Killing a PTY SESSION's whole process tree.
//!
//! Killing the child alone is never enough: an agent CLI spawns helpers, and
//! a grandchild that survives holds the PTY slave open, so the reader thread
//! never sees EOF and the pump task and the 1MB SCROLLBACK RING stay pinned
//! forever. Both platforms therefore need a *group* the child cannot leave.
//!
//! Unix gets one for free — DAEMON SETSID puts the child in its own process
//! group, and `killpg` reaches every descendant. Windows has no process
//! groups that survive a re-parent, so this module makes one out of a Job
//! Object: everything the child spawns is born into the job, and
//! `TerminateJobObject` reaches all of it at once.
//!
//! `PtySession::spawn` claims the group right after the child exists; the
//! watchdog thread in `PtySession::kill` holds a clone of it so the group
//! outlives the grace period even if the session is dropped meanwhile.

use std::sync::Arc;

/// The process tree one PTY SESSION owns, and the two questions the kill
/// watchdog asks of it: is the leader still there, and kill everything.
#[derive(Clone)]
pub struct ProcessGroup(Option<Arc<Inner>>);

impl ProcessGroup {
    /// Take ownership of a freshly spawned child and everything it will go
    /// on to spawn. `None` when the PTY gave us no pid to work with.
    pub fn claim(pid: Option<u32>) -> Self {
        Self(pid.and_then(|pid| Inner::claim(pid).map(Arc::new)))
    }

    /// True while the session leader is still running. A group we never
    /// claimed reports `false` — nothing to wait for.
    pub fn leader_alive(&self) -> bool {
        self.0.as_ref().is_some_and(|inner| inner.leader_alive())
    }

    /// Kill every process still in the group, leader included.
    pub fn kill_all(&self) {
        if let Some(inner) = &self.0 {
            inner.kill_all();
        }
    }
}

// ---------------------------------------------------------------------------
// Unix: the child's own process group, courtesy of DAEMON SETSID.
// ---------------------------------------------------------------------------
#[cfg(unix)]
struct Inner {
    pid: nix::unistd::Pid,
}

#[cfg(unix)]
impl Inner {
    fn claim(pid: u32) -> Option<Self> {
        Some(Self {
            pid: nix::unistd::Pid::from_raw(pid as i32),
        })
    }

    /// Signal 0 probes without delivering. Reaped (`ESRCH`) strictly precedes
    /// the `Exited` broadcast, so this also covers an `Exited` lost to lag.
    fn leader_alive(&self) -> bool {
        nix::sys::signal::kill(self.pid, None).is_ok()
    }

    fn kill_all(&self) {
        let _ = nix::sys::signal::killpg(self.pid, nix::sys::signal::Signal::SIGKILL);
    }
}

// ---------------------------------------------------------------------------
// Windows: a Job Object the child and its descendants cannot leave.
// ---------------------------------------------------------------------------
#[cfg(windows)]
struct Inner {
    /// The job every descendant is born into.
    job: Handle,
    /// The leader itself, kept open so its pid cannot be recycled underneath
    /// the liveness probe — a closed handle would let a brand-new process
    /// with the same pid read as "still running".
    leader: Handle,
}

/// A raw Win32 `HANDLE` that closes itself and may cross threads. The
/// watchdog runs on a plain thread, and `*mut c_void` is neither `Send` nor
/// `Sync` by default; a job/process handle is safe to use from any thread
/// (the kernel object is refcounted), so the marker is sound here.
#[cfg(windows)]
struct Handle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for Handle {}
#[cfg(windows)]
unsafe impl Sync for Handle {}

#[cfg(windows)]
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(windows)]
impl Inner {
    /// Create the job and put the child in it.
    ///
    /// The assignment can only happen *after* the spawn: `portable-pty`
    /// hands back a pid and nothing else — no process handle, and no
    /// Windows equivalent of `pre_exec` to run inside the child. So there is
    /// a window, from `CreateProcess` returning to `AssignProcessToJobObject`
    /// landing, in which a grandchild born to the child would escape the job.
    /// It is microseconds wide against agent CLIs that take tens of
    /// milliseconds to reach their first spawn, and closing it for real would
    /// mean forking `portable-pty` to spawn suspended. Accepted, recorded
    /// here, and the smoke test asserts the common case.
    fn claim(pid: u32) -> Option<Self> {
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        };
        // A standard access right, so windows-sys files it under file
        // access rights rather than with the process-specific ones.
        use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;

        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                tracing::warn!(
                    pid,
                    "CreateJobObject failed — the process tree is unmanaged"
                );
                return None;
            }
            let job = Handle(job);

            // Kill the tree when the last handle to the job closes. This is
            // what Unix gets from the PTY master's hangup: a daemon that dies
            // hard must not leave the agent CLI running headless.
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
            {
                tracing::warn!(pid, "SetInformationJobObject failed");
                return None;
            }

            let leader = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE | SYNCHRONIZE, 0, pid);
            if leader.is_null() {
                tracing::warn!(pid, "OpenProcess failed — the process tree is unmanaged");
                return None;
            }
            let leader = Handle(leader);

            if AssignProcessToJobObject(job.0, leader.0) == 0 {
                tracing::warn!(pid, "AssignProcessToJobObject failed");
                return None;
            }
            Some(Self { job, leader })
        }
    }

    /// A zero-timeout wait: `WAIT_TIMEOUT` means the process has not
    /// signalled yet, i.e. it is still running.
    fn leader_alive(&self) -> bool {
        use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
        use windows_sys::Win32::System::Threading::WaitForSingleObject;
        unsafe { WaitForSingleObject(self.leader.0, 0) == WAIT_TIMEOUT }
    }

    fn kill_all(&self) {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        unsafe { TerminateJobObject(self.job.0, 1) };
    }
}
