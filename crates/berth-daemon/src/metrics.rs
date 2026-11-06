//! Memory and CPU readings for the TUI's metrics modal and diagnostics
//! card: one machine-wide process sweep, then a per-session sum over each
//! PTY child's process subtree (an agent CLI fans out into node workers,
//! shells, MCP servers — the user cares about the whole tree, not just the
//! root).

use berth_core::{MetricsSnapshot, PrewarmInfo, SessionMetrics, SessionRef};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Cumulative CPU time per pid as of one sweep. Opaque on purpose: the only
/// thing a caller does with one is hand it back to the next sweep.
///
/// It has to be the value the immediately preceding call *of the same loop*
/// returned. A delta measured against another sweep's instant is
/// plausible-looking garbage, which is why the daemon runs exactly one
/// sampler task and keeps this as a loop local rather than a shared field.
/// It does not survive a daemon restart, and must not.
pub(crate) struct CpuTimes {
    at: Instant,
    ms: HashMap<u32, u64>,
}

/// How stale a previous sample may be and still yield a percentage. Past
/// this the denominator has stopped meaning "the recent interval".
pub(crate) const CPU_DELTA_MAX_AGE: Duration = Duration::from_secs(30);

/// Take one reading. Blocking; call via spawn_blocking.
pub(crate) fn collect(
    sessions: Vec<(SessionRef, u32, Option<PrewarmInfo>)>,
    prev: Option<&CpuTimes>,
) -> (MetricsSnapshot, CpuTimes) {
    snapshot_from_table(
        &process_table(),
        std::process::id(),
        &sessions,
        berth_core::mem::system_total_bytes().unwrap_or(0),
        prev,
        Instant::now(),
    )
}

/// Every process on the machine as `pid ppid rss-in-KB cpu-ms`, one per
/// line, where `cpu-ms` is that process's cumulative CPU time since it
/// started.
///
/// The text shape is the seam: [`snapshot_from_table`] is the tested part
/// and takes nothing else, so a platform only has to say how it enumerates
/// processes. A platform that cannot answer `cpu-ms` may leave the column
/// off; the parser reads it as 0, which comes out as "not measured".
#[cfg(target_os = "linux")]
fn process_table() -> String {
    use std::fmt::Write;

    // `ps -o time=` quantizes to whole seconds, a 20% error at a 5s tick.
    // /proc carries the real thing in clock ticks, costs no fork, and has
    // ppid and rss in the same read.
    let sysconf = |var| {
        nix::unistd::sysconf(var)
            .ok()
            .flatten()
            .filter(|v| *v > 0)
            .unwrap_or(1) as u64
    };
    let ticks_per_sec = sysconf(nix::unistd::SysconfVar::CLK_TCK);
    let page_kb = (sysconf(nix::unistd::SysconfVar::PAGE_SIZE) / 1024).max(1);
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return String::new();
    };
    let mut table = String::new();
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        // A process can exit between the readdir and the read; skip it.
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        // Field 2 is the comm, parenthesised and free to contain both
        // spaces and parens, so cut after its last ')' rather than
        // tokenizing the whole line.
        let Some((_, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        // `rest` starts at field 3 (state), so ppid is [1], utime [11],
        // stime [12] and rss-in-pages [21].
        let f: Vec<&str> = rest.split_whitespace().collect();
        let field = |i: usize| f.get(i).and_then(|s| s.parse::<u64>().ok());
        let (Some(ppid), Some(utime), Some(stime), Some(rss_pages)) =
            (field(1), field(11), field(12), field(21))
        else {
            continue;
        };
        let cpu_ms = (utime + stime) * 1000 / ticks_per_sec;
        let _ = writeln!(table, "{pid} {ppid} {} {cpu_ms}", rss_pages * page_kb);
    }
    table
}

#[cfg(all(unix, not(target_os = "linux")))]
fn process_table() -> String {
    use std::fmt::Write;

    // macOS `time=` is `[dd-]hh:mm:ss.ff` — hundredths, so 10ms.
    let out = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,rss=,time="])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let mut table = String::new();
    for line in out.lines() {
        let mut cols = line.split_whitespace();
        let (Some(pid), Some(ppid), Some(kb), Some(time)) =
            (cols.next(), cols.next(), cols.next(), cols.next())
        else {
            continue;
        };
        let _ = writeln!(table, "{pid} {ppid} {kb} {}", parse_ps_time_ms(time));
    }
    table
}

/// `[dd-]hh:mm:ss[.ff]` to milliseconds. An unparseable field reads 0,
/// which the caller turns into "not measured" rather than "idle".
#[cfg(all(unix, not(target_os = "linux")))]
fn parse_ps_time_ms(time: &str) -> u64 {
    let (days, rest) = match time.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().unwrap_or(0), rest),
        None => (0, time),
    };
    let (rest, hundredths) = match rest.split_once('.') {
        Some((r, f)) => (r, f.parse::<u64>().unwrap_or(0)),
        None => (rest, 0),
    };
    // Fields run least-significant last and the hours field is optional.
    let secs = rest
        .rsplit(':')
        .enumerate()
        .map(|(i, part)| part.parse::<u64>().unwrap_or(0) * 60u64.pow(i as u32))
        .sum::<u64>();
    (days * 86_400 + secs) * 1000 + hundredths * 10
}

/// Windows has no `ps`, and the two candidates that ship with it are both
/// worse than a crate here: `wmic` is deprecated and absent on newer builds,
/// and a PowerShell `Get-CimInstance` costs a second of shell start per poll.
#[cfg(windows)]
fn process_table() -> String {
    use std::fmt::Write;
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

    let what = ProcessRefreshKind::nothing().with_memory().with_cpu();
    let mut system = System::new_with_specifics(RefreshKind::nothing().with_processes(what));
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, what);
    let mut table = String::new();
    for (pid, process) in system.processes() {
        let ppid = process.parent().map(|p| p.as_u32()).unwrap_or(0);
        // `ps` reports KB and the parser multiplies back up, so match it
        // rather than teach the parser a second unit.
        let kb = process.memory() / 1024;
        let cpu_ms = process.accumulated_cpu_time();
        let _ = writeln!(table, "{} {ppid} {kb} {cpu_ms}", pid.as_u32());
    }
    table
}

/// Pure core, unit-testable: `table` is one process per line as
/// `pid ppid rss-KB cpu-ms`, and `now` is passed in rather than read.
fn snapshot_from_table(
    table: &str,
    daemon_pid: u32,
    sessions: &[(SessionRef, u32, Option<PrewarmInfo>)],
    system_total_bytes: u64,
    prev: Option<&CpuTimes>,
    now: Instant,
) -> (MetricsSnapshot, CpuTimes) {
    let mut rss: HashMap<u32, u64> = HashMap::new();
    let mut cpu_ms: HashMap<u32, u64> = HashMap::new();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for line in table.lines() {
        let mut cols = line.split_whitespace();
        let (Some(pid), Some(ppid), Some(kb)) = (
            cols.next().and_then(|s| s.parse::<u32>().ok()),
            cols.next().and_then(|s| s.parse::<u32>().ok()),
            cols.next().and_then(|s| s.parse::<u64>().ok()),
        ) else {
            continue;
        };
        rss.insert(pid, kb * 1024);
        // A table without the column reads 0, which yields no percentage.
        let ms = cols.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        cpu_ms.insert(pid, ms);
        children.entry(ppid).or_default().push(pid);
    }

    // A previous sample too old to divide by is no sample.
    let prev = prev.filter(|p| now.duration_since(p.at) <= CPU_DELTA_MAX_AGE);
    let elapsed_ms = prev.map(|p| now.duration_since(p.at).as_millis() as u64);

    let sessions = sessions
        .iter()
        .map(|(sref, pid, prewarm)| {
            let t = subtree_totals(*pid, &rss, &cpu_ms, &children, prev);
            SessionMetrics {
                session: sref.clone(),
                pid: *pid,
                rss_bytes: t.rss_bytes,
                procs: t.procs,
                prewarm: prewarm.clone(),
                cpu_percent: cpu_percent(t.cpu_ms, t.cpu_ms_prev, elapsed_ms),
                // Filled in by the sampler, which owns the transcript
                // tailer; a process table knows nothing about tokens.
                tokens: None,
            }
        })
        .collect();

    let snapshot = MetricsSnapshot {
        daemon_pid,
        daemon_rss_bytes: rss.get(&daemon_pid).copied().unwrap_or(0),
        system_total_bytes,
        sessions,
        sampled_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
    };
    (
        snapshot,
        CpuTimes {
            at: now,
            ms: cpu_ms,
        },
    )
}

/// Percent of one core over `elapsed_ms`. `None` whenever there is nothing
/// to subtract from — never `0.0` standing in for "not measured".
fn cpu_percent(now_ms: u64, prev_ms: u64, elapsed_ms: Option<u64>) -> Option<f32> {
    let elapsed = elapsed_ms.filter(|ms| *ms > 0)?;
    Some(now_ms.saturating_sub(prev_ms) as f32 * 100.0 / elapsed as f32)
}

/// What one subtree sums to, this sweep and the last.
struct SubtreeTotals {
    rss_bytes: u64,
    procs: u32,
    cpu_ms: u64,
    cpu_ms_prev: u64,
}

/// Sum RSS and CPU time over `root` and every descendant. The visited set
/// guards against pid-reuse cycles in a racy snapshot; a root that already
/// exited (absent from the table) reads as zero.
///
/// A pid present now and absent from `prev` — a worker spawned
/// mid-interval — contributes its whole `cpu_ms`, which is right: it did
/// that work inside the interval.
fn subtree_totals(
    root: u32,
    rss: &HashMap<u32, u64>,
    cpu_ms: &HashMap<u32, u64>,
    children: &HashMap<u32, Vec<u32>>,
    prev: Option<&CpuTimes>,
) -> SubtreeTotals {
    let mut stack = vec![root];
    let mut seen = HashSet::new();
    let mut out = SubtreeTotals {
        rss_bytes: 0,
        procs: 0,
        cpu_ms: 0,
        cpu_ms_prev: 0,
    };
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        if let Some(b) = rss.get(&pid) {
            out.rss_bytes += b;
            out.procs += 1;
        }
        if let Some(ms) = cpu_ms.get(&pid) {
            out.cpu_ms += ms;
            out.cpu_ms_prev += prev.and_then(|p| p.ms.get(&pid)).copied().unwrap_or(0);
        }
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use berth_core::{AgentId, TerminalId};

    fn sref(id: &str) -> SessionRef {
        SessionRef::Agent(AgentId(id.into()))
    }

    /// One sweep with no history: what the RSS tests care about.
    fn parse(
        table: &str,
        daemon_pid: u32,
        sessions: &[(SessionRef, u32, Option<PrewarmInfo>)],
        system_total_bytes: u64,
    ) -> MetricsSnapshot {
        snapshot_from_table(
            table,
            daemon_pid,
            sessions,
            system_total_bytes,
            None,
            Instant::now(),
        )
        .0
    }

    #[test]
    fn sums_each_sessions_whole_subtree() {
        // daemon 10 → session roots 20 and 30; 20 → {21, 22}, 22 → 23.
        // 99 is an unrelated process and must count nowhere.
        let table = "\
 10     1  1000
 20    10  2000
 21    20   300
 22    20   500
 23    22   200
 30    10    50
 99     1  9999
";
        let snap = parse(
            table,
            10,
            &[
                (sref("a"), 20, None),
                (SessionRef::Terminal(TerminalId("t".into())), 30, None),
            ],
            0,
        );
        assert_eq!(snap.daemon_rss_bytes, 1000 * 1024);
        assert_eq!(snap.sessions[0].rss_bytes, (2000 + 300 + 500 + 200) * 1024);
        assert_eq!(snap.sessions[0].procs, 4);
        assert_eq!(snap.sessions[1].rss_bytes, 50 * 1024);
        assert_eq!(snap.sessions[1].procs, 1);
    }

    #[test]
    fn exited_session_reads_zero() {
        let snap = parse(" 10 1 100\n", 10, &[(sref("a"), 555, None)], 0);
        assert_eq!(snap.sessions[0].rss_bytes, 0);
        assert_eq!(snap.sessions[0].procs, 0);
    }

    #[test]
    fn garbage_lines_are_skipped() {
        let table = "not numbers at all\n 20 10 100\n";
        let snap = parse(table, 10, &[(sref("a"), 20, None)], 0);
        assert_eq!(snap.sessions[0].rss_bytes, 100 * 1024);
        assert_eq!(snap.daemon_rss_bytes, 0);
    }

    /// A pool spare's home rides along untouched: the client has no agent
    /// row to look it up by, so this is what it labels the row from.
    #[test]
    fn prewarm_home_passes_through() {
        use berth_core::{AgentKind, WorktreeId};
        let home = PrewarmInfo {
            worktree: WorktreeId("w1".into()),
            kind: AgentKind::Claude,
            model: Some("opus".into()),
        };
        let snap = parse(
            " 20 10 100\n 21 10 50\n",
            10,
            &[
                (sref("warm"), 20, Some(home.clone())),
                (sref("live"), 21, None),
            ],
            0,
        );
        assert_eq!(snap.sessions[0].prewarm.as_ref(), Some(&home));
        assert_eq!(snap.sessions[1].prewarm, None);
    }

    /// The percentage is a subtree delta over the gap between two sweeps,
    /// and "not measured" never renders as idle. Catches an edit dividing
    /// by the wrong clock, and one substituting 0.0 for None.
    #[test]
    fn cpu_percent_is_a_subtree_delta_over_the_gap() {
        // 20 → 21 is one session's subtree; 30 is another's, and idle.
        let before = " 10 1 100 0\n 20 10 100 1000\n 21 20 100 500\n 30 10 100 200\n";
        let after = " 10 1 100 0\n 20 10 100 1300\n 21 20 100 700\n 30 10 100 200\n";
        let sessions = [(sref("busy"), 20, None), (sref("idle"), 30, None)];

        let t0 = Instant::now();
        let (first, prev) = snapshot_from_table(before, 10, &sessions, 0, None, t0);
        // Nothing to subtract from on the first sweep.
        assert_eq!(first.sessions[0].cpu_percent, None);
        assert_eq!(first.sessions[1].cpu_percent, None);

        let (second, _) = snapshot_from_table(
            after,
            10,
            &sessions,
            0,
            Some(&prev),
            t0 + Duration::from_secs(1),
        );
        // (300 + 200) ms of CPU over 1000 ms of wall clock.
        assert_eq!(second.sessions[0].cpu_percent, Some(50.0));
        assert_eq!(second.sessions[1].cpu_percent, Some(0.0));
    }

    #[test]
    fn a_stale_previous_sample_yields_no_percentage() {
        let table = " 20 10 100 1000\n";
        let sessions = [(sref("a"), 20, None)];
        let t0 = Instant::now();
        let (_, prev) = snapshot_from_table(table, 10, &sessions, 0, None, t0);
        let too_late = t0 + CPU_DELTA_MAX_AGE + Duration::from_secs(1);
        let (snap, _) = snapshot_from_table(table, 10, &sessions, 0, Some(&prev), too_late);
        assert_eq!(snap.sessions[0].cpu_percent, None);
    }
}
