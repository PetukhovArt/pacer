//! Claude token spend, read out of the CLI's own transcripts.
//!
//! The daemon never sees usage data any other way: hook payloads carry no
//! token fields, and nothing else on the machine records what a turn cost
//! (`stats-cache.json` has a `costUSD` and it is 0 on a subscription plan).
//! So this tails each live Claude session's `~/.claude/projects/<slug>/
//! <session-id>.jsonl`, summing `message.usage` over the assistant lines,
//! reading only the bytes appended since the last pass.
//!
//! **`output_tokens_details.thinking_tokens` is deliberately ignored.** It
//! is a breakdown *inside* `output_tokens`, not an addition to it; adding
//! it double-bills. Reasoning effort therefore needs no coefficient
//! anywhere — it shows up as more output tokens on its own. This is the
//! exact mistake the next reader is about to make.
//!
//! Likewise `usage.iterations[]` repeats the same numbers a second time,
//! and the flat `cache_creation_input_tokens` repeats what the nested
//! `cache_creation` object splits into tiers. Only the top-level fields
//! below are read.
//!
//! The same pass picks up the title a user gives the conversation with
//! Claude's `/rename` (a `custom-title` line), since it already reads every
//! appended byte.

use pacer_core::{AgentId, AgentKind, ModelTokens, TokenUsage};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// What the tailer needs to find and attribute one conversation.
pub(crate) struct AgentTranscript {
    pub agent: AgentId,
    pub kind: AgentKind,
    /// Where the session runs (its worktree's path).
    pub cwd: PathBuf,
    /// CLI session id; None until the first hook lands.
    pub session_id: Option<String>,
    /// The path Claude itself reported on a hook. Authoritative when it
    /// names this session's id.
    pub hinted: Option<PathBuf>,
}

/// Stateful, single-owner tailer. One instance, owned by the daemon's
/// sampler loop — the byte offsets only mean anything in sequence.
#[derive(Default)]
pub(crate) struct UsageTailer {
    /// Keyed by agent, never by path: a Resume gives a new session id and
    /// so a new path, and that is a *different* conversation whose total
    /// starts at zero. Keying by path would both leak an entry per
    /// conversation for the daemon's life and carry the old total over.
    by_agent: HashMap<AgentId, Cursor>,
}

/// Where one conversation has been read to, and what it came to.
struct Cursor {
    session_id: String,
    path: PathBuf,
    offset: u64,
    total: TokenUsage,
    /// The last `/rename` title seen. Claude re-appends the same line all
    /// the time, and a new cursor (daemon restart, respawn, resume) re-reads
    /// old ones, so only a change after the first read counts as a rename —
    /// otherwise a name set in pacer since would be overwritten.
    title: Option<String>,
    /// Whether the first read happened, which only sets the baseline title.
    primed: bool,
}

#[derive(Default)]
pub(crate) struct Sweep {
    pub tokens: HashMap<AgentId, TokenUsage>,
    /// Conversations the user retitled with `/rename` since the last pass.
    pub titles: Vec<(AgentId, String)>,
}

impl UsageTailer {
    /// One pass, reading only the bytes appended since the last one.
    /// Blocking; call via spawn_blocking.
    ///
    /// An agent absent from `tokens` was **not measured** — a Codex or
    /// Cursor session, one with no session id yet, or one whose transcript
    /// could not be located. Never read that as zero. An agent measured
    /// with nothing spent is present with an empty `by_model`.
    pub(crate) fn sweep(&mut self, agents: &[AgentTranscript]) -> Sweep {
        let mut out = Sweep::default();
        for agent in agents {
            if agent.kind != AgentKind::Claude {
                continue;
            }
            let Some(session_id) = agent.session_id.as_deref() else {
                continue;
            };
            // A resume replaced the conversation: drop what the old one
            // came to rather than keep adding to it.
            let stale = self
                .by_agent
                .get(&agent.agent)
                .is_some_and(|c| c.session_id != session_id);
            if stale {
                self.by_agent.remove(&agent.agent);
            }
            if !self.by_agent.contains_key(&agent.agent) {
                // Resolution is the expensive step, so it happens once per
                // conversation and the answer lives in the cursor.
                let Some(path) = resolve(agent, session_id) else {
                    continue;
                };
                self.by_agent.insert(
                    agent.agent.clone(),
                    Cursor {
                        session_id: session_id.to_string(),
                        path,
                        offset: 0,
                        total: TokenUsage::default(),
                        title: None,
                        primed: false,
                    },
                );
            }
            let cursor = self.by_agent.get_mut(&agent.agent).expect("just inserted");
            let seen = cursor.title.clone();
            if !advance(cursor) {
                continue;
            }
            out.tokens.insert(agent.agent.clone(), cursor.total.clone());
            if cursor.primed && cursor.title != seen {
                if let Some(title) = &cursor.title {
                    out.titles.push((agent.agent.clone(), title.clone()));
                }
            }
            cursor.primed = true;
        }
        // Agents that went away take their cursors with them.
        self.by_agent
            .retain(|id, _| agents.iter().any(|a| &a.agent == id));
        out
    }
}

/// Cheapest and most authoritative first. The hint has to name this
/// session's id to be used: it is the *last* path the hooks reported, and
/// a resume moves on before the next hook lands.
fn resolve(agent: &AgentTranscript, session_id: &str) -> Option<PathBuf> {
    let stem_matches = |p: &Path| p.file_stem().is_some_and(|s| s == session_id);
    agent
        .hinted
        .clone()
        .filter(|p| stem_matches(p) && p.exists())
        .or_else(|| crate::orphans::transcript_path(&agent.cwd, session_id))
        .or_else(|| crate::orphans::find_transcript(session_id))
}

/// Read whatever was appended and fold it into the running total. Returns
/// whether the file could be read at all — a false is "not measured", which
/// is not the same as a total of zero.
fn advance(cursor: &mut Cursor) -> bool {
    let Ok(mut file) = std::fs::File::open(&cursor.path) else {
        // Claude creates the transcript at session start, so this means the
        // path is wrong, not that nothing was spent.
        return false;
    };
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return false;
    };
    if len < cursor.offset {
        // A transcript is append-only, so a shrink means a different file
        // now sits at that path (a `/clear`, a compaction rewrite).
        cursor.offset = 0;
        cursor.total = TokenUsage::default();
    }
    if len == cursor.offset {
        return true;
    }
    if file.seek(SeekFrom::Start(cursor.offset)).is_err() {
        return false;
    }
    let mut chunk = Vec::new();
    if file.read_to_end(&mut chunk).is_err() {
        return false;
    }
    let (add, title, consumed) = tally(&chunk);
    for model in &add.by_model {
        cursor.total.add(&model.model, model);
    }
    if title.is_some() {
        cursor.title = title;
    }
    cursor.offset += consumed as u64;
    true
}

/// Sum `message.usage` over every assistant line in `chunk`. Returns the
/// tally, the last `/rename` title, and how many bytes were consumed: a
/// trailing partial line is **not** consumed, so the next sweep re-reads it
/// whole.
///
/// That rule is what makes incremental tailing correct at all — Claude
/// appends while this reads, so the last line is routinely half-written.
/// Bytes rather than `&str` for the same reason: a chunk can end mid
/// codepoint, and a lossy conversion would move the byte offsets the rule
/// depends on.
fn tally(chunk: &[u8]) -> (TokenUsage, Option<String>, usize) {
    let Some(last) = chunk.iter().rposition(|b| *b == b'\n') else {
        return (TokenUsage::default(), None, 0);
    };
    let consumed = last + 1;
    let mut out = TokenUsage::default();
    let mut title = None;
    for line in chunk[..consumed].split(|b| *b == b'\n') {
        // A malformed line is skipped, not fatal: the fail-soft posture the
        // other transcript reader already takes.
        let Ok(entry) = serde_json::from_slice::<Entry>(line) else {
            continue;
        };
        if entry.r#type == "custom-title" {
            if let Some(t) = entry.custom_title.filter(|t| !t.trim().is_empty()) {
                title = Some(t);
            }
            continue;
        }
        if entry.r#type != "assistant" {
            continue;
        }
        let Some(message) = entry.message else {
            continue;
        };
        let (Some(model), Some(usage)) = (message.model, message.usage) else {
            continue;
        };
        // Claude Code's own placeholder turns, never billed.
        if model == "<synthetic>" {
            continue;
        }
        out.add(&model, &usage.into_tokens());
    }
    (out, title, consumed)
}

#[derive(Deserialize)]
struct Entry {
    #[serde(default)]
    r#type: String,
    message: Option<Message>,
    #[serde(rename = "customTitle")]
    custom_title: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    model: Option<String>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    /// Present on older lines only, and superseded by `cache_creation`
    /// when both are there — the two report the same total.
    #[serde(default)]
    cache_creation_input_tokens: u64,
    cache_creation: Option<CacheCreation>,
}

#[derive(Deserialize)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

impl Usage {
    fn into_tokens(self) -> ModelTokens {
        // The two cache write tiers price differently, so they stay apart
        // all the way to the price table. A line carrying only the legacy
        // flat field folds into 5m, Claude's default TTL.
        let (write_5m, write_1h) = match self.cache_creation {
            Some(c) => (c.ephemeral_5m_input_tokens, c.ephemeral_1h_input_tokens),
            None => (self.cache_creation_input_tokens, 0),
        };
        ModelTokens {
            model: String::new(),
            input: self.input_tokens,
            output: self.output_tokens,
            cache_read: self.cache_read_input_tokens,
            cache_write_5m: write_5m,
            cache_write_1h: write_1h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn assistant(model: &str, input: u64, output: u64, cache: &str) -> String {
        format!(
            r#"{{"type":"assistant","message":{{"model":"{model}","usage":{{"input_tokens":{input},"output_tokens":{output},"cache_read_input_tokens":7,{cache}}}}}}}"#
        )
    }

    const NESTED: &str = r#""cache_creation_input_tokens":300,"cache_creation":{"ephemeral_5m_input_tokens":100,"ephemeral_1h_input_tokens":200}"#;
    const LEGACY: &str = r#""cache_creation_input_tokens":50"#;

    fn tokens_of<'a>(usage: &'a TokenUsage, model: &str) -> &'a ModelTokens {
        usage
            .by_model
            .iter()
            .find(|m| m.model == model)
            .unwrap_or_else(|| panic!("no entry for {model} in {:?}", usage.by_model))
    }

    /// The field names Claude Code writes, and the partial-line rule that
    /// makes tailing an actively-appended file correct.
    #[test]
    fn tallies_assistant_usage_and_stops_before_a_partial_line() {
        let mut chunk = String::new();
        chunk.push_str(&assistant("claude-opus-5", 10, 20, NESTED));
        chunk.push('\n');
        chunk.push_str(&assistant("claude-opus-5", 1, 2, LEGACY));
        chunk.push('\n');
        chunk.push_str(&assistant("<synthetic>", 999, 999, LEGACY));
        chunk.push('\n');
        chunk.push_str(r#"{"type":"user","message":{"role":"user"}}"#);
        chunk.push('\n');
        chunk.push_str("{not json at all");
        chunk.push('\n');
        let complete = chunk.len();
        // Half an appended line, as a live transcript routinely ends.
        chunk.push_str(&assistant("claude-opus-5", 500, 500, LEGACY)[..20]);

        let (usage, _, consumed) = tally(chunk.as_bytes());
        assert_eq!(consumed, complete, "a partial line must not be consumed");
        assert_eq!(usage.by_model.len(), 1, "synthetic and user lines counted");
        let opus = tokens_of(&usage, "claude-opus-5");
        assert_eq!(opus.input, 11);
        assert_eq!(opus.output, 22);
        assert_eq!(opus.cache_read, 14);
        // The nested split wins over the flat field on the first line; the
        // second has only the flat one, which is a 5m write.
        assert_eq!(opus.cache_write_5m, 150);
        assert_eq!(opus.cache_write_1h, 200);
    }

    #[test]
    fn a_chunk_with_no_newline_consumes_nothing() {
        let (usage, _, consumed) = tally(assistant("claude-opus-5", 1, 1, LEGACY).as_bytes());
        assert_eq!(consumed, 0);
        assert!(usage.by_model.is_empty());
    }

    fn write(path: &Path, text: &str) {
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(text.as_bytes()).unwrap();
    }

    fn agent_at(path: &Path, session_id: &str) -> AgentTranscript {
        AgentTranscript {
            agent: AgentId("a1".into()),
            kind: AgentKind::Claude,
            cwd: PathBuf::from("/nowhere"),
            session_id: Some(session_id.into()),
            hinted: Some(path.to_path_buf()),
        }
    }

    /// The three state transitions that decide whether a total is right:
    /// a grown file adds only its tail, a shrunk one starts over, and a
    /// resume is a different conversation.
    #[test]
    fn sweeps_are_incremental_and_reset_on_a_new_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let sid = "11111111-1111-1111-1111-111111111111";
        let path = dir.path().join(format!("{sid}.jsonl"));
        write(&path, &format!("{}\n", assistant("m", 10, 0, LEGACY)));

        let mut tailer = UsageTailer::default();
        let agents = [agent_at(&path, sid)];
        let first = tailer.sweep(&agents);
        assert_eq!(
            tokens_of(&first.tokens[&AgentId("a1".into())], "m").input,
            10
        );

        // Grown: the tail only, not the whole file again.
        write(
            &path,
            &format!(
                "{}\n{}\n",
                assistant("m", 10, 0, LEGACY),
                assistant("m", 5, 0, LEGACY)
            ),
        );
        let second = tailer.sweep(&agents);
        assert_eq!(
            tokens_of(&second.tokens[&AgentId("a1".into())], "m").input,
            15
        );

        // Shrunk: a different file now sits at that path.
        write(&path, &format!("{}\n", assistant("m", 3, 0, LEGACY)));
        let third = tailer.sweep(&agents);
        assert_eq!(
            tokens_of(&third.tokens[&AgentId("a1".into())], "m").input,
            3
        );

        // Resumed: a new session id is a new conversation, from zero.
        let sid2 = "22222222-2222-2222-2222-222222222222";
        let path2 = dir.path().join(format!("{sid2}.jsonl"));
        write(&path2, &format!("{}\n", assistant("m", 1, 0, LEGACY)));
        let fourth = tailer.sweep(&[agent_at(&path2, sid2)]);
        assert_eq!(
            tokens_of(&fourth.tokens[&AgentId("a1".into())], "m").input,
            1
        );
    }

    /// Absent and zero are different facts, and the card renders them
    /// differently.
    #[test]
    fn unmeasurable_agents_are_absent_rather_than_zero() {
        let dir = tempfile::tempdir().unwrap();
        let sid = "33333333-3333-3333-3333-333333333333";
        let empty = dir.path().join(format!("{sid}.jsonl"));
        write(&empty, "");

        let mut tailer = UsageTailer::default();
        let measured = tailer.sweep(&[agent_at(&empty, sid)]);
        assert_eq!(
            measured.tokens[&AgentId("a1".into())],
            TokenUsage::default(),
            "a started conversation with no turn is measured, and spent nothing"
        );

        let no_session = AgentTranscript {
            session_id: None,
            ..agent_at(&empty, sid)
        };
        assert!(tailer.sweep(&[no_session]).tokens.is_empty());

        let codex = AgentTranscript {
            kind: AgentKind::Codex,
            ..agent_at(&empty, sid)
        };
        assert!(tailer.sweep(&[codex]).tokens.is_empty());
    }

    /// A title already in the transcript, or repeated by Claude, must not
    /// undo a name given in pacer since — only a new `/rename` counts.
    #[test]
    fn only_a_new_rename_is_reported_not_a_reread_or_repeated_one() {
        let dir = tempfile::tempdir().unwrap();
        let sid = "44444444-4444-4444-4444-444444444444";
        let path = dir.path().join(format!("{sid}.jsonl"));
        let titled = |t: &str| format!(r#"{{"type":"custom-title","customTitle":"{t}"}}"#);
        let a1 = AgentId("a1".into());
        let mut log = String::new();
        let mut grow = |line: String| {
            log.push_str(&line);
            log.push('\n');
            write(&path, &log);
        };

        // A daemon restart reads an old rename with a fresh tailer.
        grow(titled("Old"));
        let mut tailer = UsageTailer::default();
        assert!(tailer.sweep(&[agent_at(&path, sid)]).titles.is_empty());

        grow(assistant("m", 1, 0, LEGACY));
        grow(titled("Old"));
        assert!(tailer.sweep(&[agent_at(&path, sid)]).titles.is_empty());

        grow(titled("Fix Login"));
        let renamed = tailer.sweep(&[agent_at(&path, sid)]);
        assert_eq!(renamed.titles, [(a1, "Fix Login".to_string())]);

        // A resume starts a new transcript that repeats the title.
        let sid2 = "55555555-5555-5555-5555-555555555555";
        let path2 = dir.path().join(format!("{sid2}.jsonl"));
        write(&path2, &format!("{}\n", titled("Fix Login")));
        assert!(tailer.sweep(&[agent_at(&path2, sid2)]).titles.is_empty());
    }
}
