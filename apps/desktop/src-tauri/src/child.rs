use std::io;

use tokio::process::{Child, Command};

#[cfg(windows)]
const NO_WINDOW: u32 = 0x0800_0000;

pub fn spawn(command: &mut Command) -> io::Result<Child> {
    prepare(command);
    let child = command.spawn()?;
    bind(&child);
    Ok(child)
}

#[cfg(windows)]
fn prepare(command: &mut Command) {
    command.creation_flags(NO_WINDOW);
}

#[cfg(unix)]
fn prepare(command: &mut Command) {
    command.process_group(0);
    #[cfg(target_os = "linux")]
    {
        let parent = std::process::id() as libc::pid_t;
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong) == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    libc::_exit(1);
                }
                Ok(())
            });
        }
    }
}

#[cfg(windows)]
fn bind(child: &Child) {
    let (Some(job), Some(process)) = (job::handle(), child.raw_handle()) else {
        return;
    };
    unsafe {
        windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(job, process);
    }
}

#[cfg(not(windows))]
fn bind(_child: &Child) {}

#[cfg(windows)]
mod job {
    use std::sync::OnceLock;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    static JOB: OnceLock<usize> = OnceLock::new();

    pub fn handle() -> Option<HANDLE> {
        let raw = *JOB.get_or_init(create);
        (raw != 0).then_some(raw as HANDLE)
    }

    fn create() -> usize {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if set == 0 {
                return 0;
            }
            job as usize
        }
    }
}
