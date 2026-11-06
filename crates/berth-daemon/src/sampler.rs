//! The one owner of the daemon's stateful measurements.
//!
//! CPU is a delta between two sweeps and the token tally is a byte offset
//! into a file, so both only mean anything in sequence. `GetMetrics` is
//! handled per client connection in a detached task, so measuring on
//! request would date every delta from whichever client swept last — and
//! multiply the transcript I/O by however many clients happen to be
//! attached. A measurement whose correctness depends on that is not a
//! measurement.
//!
//! So this loop measures, and [`Daemon::last_sample`] is what a request
//! reads. Both hazards close structurally rather than by locking:
//! [`CpuTimes`] and [`UsageTailer`] are locals of this loop, so there is no
//! sharing to get wrong, and the loop is the only writer of the published
//! sample, so ordering is its own program order.
//!
//! The transcript pass also reports `/rename` titles, so this loop applies
//! them to the agent rows as well.

use crate::metrics::CpuTimes;
use crate::registry::Daemon;
use crate::usage::{AgentTranscript, UsageTailer};
use berth_core::{env, AgentId, AgentKind, SessionRef, TokenUsage};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// How often the process table is swept. A CPU delta wants a regular,
/// not-too-long interval; this matches the TUI's own metrics poll.
const TICK: Duration = Duration::from_secs(5);

/// Transcripts are re-read every Nth tick. A token total is monotone and
/// nobody notices 15s of staleness, while a CPU delta is meaningless if
/// irregular — so the two cadences differ inside one loop rather than
/// across two, which would let the card's two columns disagree about which
/// sessions exist.
const TOKENS_EVERY: u32 = 3;

pub async fn run(daemon: Arc<Daemon>) {
    let mut interval = crate::env_interval(env::METRICS_SAMPLE_MS, TICK.as_millis() as u64);
    let mut cpu: Option<CpuTimes> = None;
    let mut tailer = UsageTailer::default();
    // The token sweep runs on its own slower cadence, so the ticks between
    // two of them re-publish the last tally rather than dropping the
    // column to "not measured" and back.
    let mut last_tokens: HashMap<AgentId, TokenUsage> = HashMap::new();
    let mut tick: u32 = 0;
    loop {
        tokio::select! {
            _ = daemon.shutdown.cancelled() => break,
            _ = interval.tick() => {}
        }
        // Nobody is looking, so this would be pure new work. A client that
        // attaches waits one tick for its first CPU reading, which it was
        // going to do anyway — the first sweep has nothing to subtract.
        if daemon.events.receiver_count() == 0 {
            cpu = None;
            continue;
        }

        let pids = daemon.session_pids();
        // `sweep` takes the live agents as its whole truth and forgets the
        // cursors of everyone absent, so it must not be called on a tick
        // that has no list to give it: an empty slice reads as "every
        // session ended" and throws away every byte offset, which would
        // re-read each transcript from zero on the next real sweep.
        let transcripts = tick
            .is_multiple_of(TOKENS_EVERY)
            .then(|| transcript_inputs(&daemon, &pids));
        tick = tick.wrapping_add(1);

        // One blocking hop for both: `ps` and the transcript reads are
        // filesystem work, and keeping them in one task keeps the row set
        // coherent inside the published snapshot.
        let prev = cpu.take();
        let mut lent = std::mem::take(&mut tailer);
        let handle = tokio::task::spawn_blocking(move || {
            let (snapshot, next) = crate::metrics::collect(pids, prev.as_ref());
            let tokens = transcripts.map(|t| lent.sweep(&t));
            (snapshot, next, lent, tokens)
        });
        let (mut snapshot, next, returned, swept) = match handle.await {
            Ok(done) => done,
            Err(err) => {
                // The tailer went into the task and died with it, so the
                // next sweep starts every cursor over and re-reads each
                // transcript whole. Correct, but expensive and otherwise
                // invisible.
                tracing::warn!(error = %err, "metrics sample failed; token cursors reset");
                continue;
            }
        };
        cpu = Some(next);
        tailer = returned;
        if let Some(swept) = swept {
            // A `/rename` typed in Claude is a name a human chose, so it
            // replaces the row's name the way `r` in berth does.
            for (id, title) in &swept.titles {
                if let Err(e) = daemon.rename_agent(id, title) {
                    tracing::warn!(agent = %id, error = %e, "adopting /rename title failed");
                }
            }
            last_tokens = swept.tokens;
        }
        for session in &mut snapshot.sessions {
            if let SessionRef::Agent(id) = &session.session {
                session.tokens = last_tokens.get(id).cloned();
            }
        }
        daemon.publish_sample(snapshot);
    }
}

/// What the tailer needs to find each live Claude conversation: where the
/// session runs, its CLI session id, and the path the hooks reported.
fn transcript_inputs(
    daemon: &Daemon,
    pids: &[(SessionRef, u32, Option<berth_core::PrewarmInfo>)],
) -> Vec<AgentTranscript> {
    // Point lookups per live agent, not `load_tree`: this runs on a timer
    // forever, and a handful of running sessions should not cost a scan of
    // every project, worktree and terminal the install ever had.
    pids.iter()
        .filter_map(|(sref, _, _)| match sref {
            SessionRef::Agent(id) => Some(id),
            SessionRef::Terminal(_) => None,
        })
        .filter_map(|id| {
            let agent = daemon.store.get_agent(id).ok().flatten()?;
            if agent.kind != AgentKind::Claude {
                return None;
            }
            let cwd = daemon
                .store
                .get_worktree(&agent.worktree_id)
                .ok()
                .flatten()
                .map(|w| w.path)?;
            Some(AgentTranscript {
                agent: id.clone(),
                kind: agent.kind,
                cwd,
                session_id: agent.session_id.clone(),
                hinted: daemon.transcript_path(id),
            })
        })
        .collect()
}
