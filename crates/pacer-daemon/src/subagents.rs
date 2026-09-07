//! Which background subagents Claude killed without a `SubagentStop`.
//!
//! Esc-cancelling a turn, or `TaskStop` on a background Agent-tool worker,
//! kills it and fires no hook at all. [`crate::status::AgentStatusMachine`]
//! therefore keeps it tracked and the STOP GATE holds the row on `running`
//! until `SUBAGENT_QUIET_GRACE` — half an hour of a session sweeping yellow
//! after it answered. Claude does record the kill: it stamps
//! `"stoppedByUser": true` into the worker's `.meta.json` beside its
//! transcript.
//!
//! This is a Claude-only accelerator, not a replacement: codex and
//! cursor-agent write no such file, and for them (and for a Claude session
//! whose transcript path we never saw) the quiet grace stays the floor.
//!
//! The status machine stays pure — it never reads a file. It reports which
//! ids a hold is waiting on ([`crate::status::AgentStatusMachine::held_subagents`]),
//! the registry's tick asks here which of those are dead, and feeds the
//! answer back in via `forget_subagent`.

use pacer_core::AgentId;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Meta {
    #[serde(rename = "stoppedByUser")]
    stopped_by_user: bool,
}

/// Every session's last known transcript path, learned from the hook
/// payloads. Not persisted: after a daemon restart a session's holds fall
/// back to the quiet grace until its next hook arrives.
#[derive(Default)]
pub struct KilledSubagents {
    transcripts: Mutex<HashMap<AgentId, PathBuf>>,
}

impl KilledSubagents {
    pub fn note_transcript(&self, agent_id: &AgentId, path: &str) {
        self.transcripts
            .lock()
            .unwrap()
            .insert(agent_id.clone(), PathBuf::from(path));
    }

    /// Ids among `tracked` that this session's meta files report as killed.
    /// Empty whenever the path is unknown or nothing on disk says so.
    pub fn killed(&self, agent_id: &AgentId, tracked: &[String]) -> Vec<String> {
        let Some(transcript) = self.transcripts.lock().unwrap().get(agent_id).cloned() else {
            return Vec::new();
        };
        let dir = subagents_dir(&transcript);
        tracked
            .iter()
            .filter(|id| stopped_by_user(&dir, id))
            .cloned()
            .collect()
    }
}

/// `…/<session>.jsonl` -> `…/<session>/subagents`, where Claude keeps one
/// `agent-<id>.jsonl` and `agent-<id>.meta.json` per background worker.
fn subagents_dir(transcript: &Path) -> PathBuf {
    transcript.with_extension("").join("subagents")
}

fn stopped_by_user(dir: &Path, id: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join(format!("agent-{id}.meta.json"))) else {
        return false;
    };
    serde_json::from_str::<Meta>(&text)
        .map(|m| m.stopped_by_user)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, id: &str, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(format!("agent-{id}.meta.json")), body).unwrap();
    }

    /// The contract with Claude's on-disk layout: this is what tells a
    /// killed worker from a live one, and nothing in our own code would
    /// fail if the path shape or the flag name drifted.
    #[test]
    fn reads_the_kill_flag_out_of_claudes_meta_files() {
        let tmp = tempfile::tempdir().unwrap();
        let transcript = tmp.path().join("s1.jsonl");
        let dir = subagents_dir(&transcript);
        write(
            &dir,
            "dead",
            r#"{"agentType":"general-purpose","stoppedByUser":true}"#,
        );
        write(&dir, "live", r#"{"agentType":"general-purpose"}"#);

        let killed = KilledSubagents::default();
        killed.note_transcript(&AgentId("a1".into()), transcript.to_str().unwrap());

        let tracked = vec!["dead".to_string(), "live".to_string(), "absent".to_string()];
        assert_eq!(killed.killed(&AgentId("a1".into()), &tracked), vec!["dead"]);
        // An agent we never saw a transcript for reports nothing.
        assert!(killed.killed(&AgentId("a2".into()), &tracked).is_empty());
    }
}
