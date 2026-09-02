//! Small memory probes shared by the daemon and the TUI client. They only run
//! on the metrics modal's slow poll, never on a hot path — which is why the
//! Unix ones are free to shell out (macOS has no /proc).

/// Resident set size of one process, bytes.
#[cfg(unix)]
pub fn process_rss_bytes(pid: u32) -> Option<u64> {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    text.trim().parse::<u64>().ok().map(|kb| kb * 1024)
}

/// The working set is Windows' name for the resident set: the process's
/// pages currently in physical memory, which is the number the metrics modal
/// means by RSS everywhere else.
#[cfg(windows)]
pub fn process_rss_bytes(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let ok = GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ) != 0;
        CloseHandle(handle);
        ok.then_some(counters.WorkingSetSize as u64)
    }
}

/// Physical memory installed on this machine, bytes.
#[cfg(target_os = "macos")]
pub fn system_total_bytes() -> Option<u64> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn system_total_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(windows)]
pub fn system_total_bytes() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        // The struct carries its own size so the OS knows which version it
        // was handed; zeroing it leaves that field wrong.
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        (GlobalMemoryStatusEx(&mut status) != 0).then_some(status.ullTotalPhys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both probes answer for *this* process and *this* machine, so a plain
    /// sanity check is available on every platform: a running process has a
    /// non-zero resident set, and the machine has some RAM installed.
    #[test]
    fn this_process_and_this_machine_report_plausible_memory() {
        let rss = process_rss_bytes(std::process::id());
        assert!(
            rss.is_some_and(|b| b > 0),
            "a running process has a resident set: {rss:?}"
        );
        let total = system_total_bytes();
        assert!(
            total.is_some_and(|b| b > 64 * 1024 * 1024),
            "the machine has RAM: {total:?}"
        );
    }
}
