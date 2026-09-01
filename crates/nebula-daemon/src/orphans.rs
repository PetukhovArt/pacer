//! ORPHANED SESSIONS: the conversations whose WORKTREE was deleted.
//!
//! Two sources, deliberately unequal. The store's `orphaned_sessions` table
//! is authoritative — every AGENT KIND, the name the user gave the row —
//! but it only knows what nebula copied aside before the delete cascade, so
//! it is blind to everything orphaned before that mechanism existed. The
//! agent CLI's own transcript store fills that in: it survived the delete
//! untouched, and it reaches back over the whole history. It is Claude-only
//! (codex keeps conversations in a SQLite of its own, cursor-agent in
//! nothing this can read), and it knows a title and a branch rather than a
//! session row.
//!
//! Neither source is trusted to be complete, so the merge is a union keyed
//! by CLI session id, with the store row winning any field both can answer.

use anyhow::Result;
use nebula_core::{paths, OrphanedSession, Project, Worktree};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::store::Store;

/// How far into a transcript to look for its metadata. The `cwd` and
/// `gitBranch` ride on the first real message, behind a handful of tiny
/// header lines — but that message can be a whole pasted file, so the budget
/// is a byte count rather than a line count, and the scan stops the moment
/// it has what it needs.
const HEAD_BUDGET_BYTES: u64 = 512 * 1024;

/// Every ORPHANED SESSION of `project`, newest first.
///
/// `live_worktrees` is the project's current checkouts: a transcript whose
/// working directory still sits inside one of them is not orphaned — that
/// conversation is reachable the ordinary way, from its own session row.
pub fn list(
    store: &Store,
    project: &Project,
    live_worktrees: &[Worktree],
) -> Result<Vec<OrphanedSession>> {
    let stored = store.load_orphaned_sessions(&project.id)?;
    let scanned = match claude_projects_dir() {
        Some(root) => scan_claude_transcripts(&root, project, live_worktrees),
        None => Vec::new(),
    };
    Ok(merge(stored, scanned))
}

/// Union the two sources by CLI session id, newest first.
///
/// The store row wins every field both can answer: it carries the AGENT KIND
/// and the name the user actually gave the session, where the transcript can
/// only offer the CLI's own title. The transcript still contributes the one
/// thing no column knows — that the conversation is still on disk, and how
/// big it grew.
fn merge(mut stored: Vec<OrphanedSession>, scanned: Vec<OrphanedSession>) -> Vec<OrphanedSession> {
    let mut seen: HashMap<String, usize> = stored
        .iter()
        .enumerate()
        .map(|(i, o)| (o.session_id.clone(), i))
        .collect();
    for found in scanned {
        match seen.get(&found.session_id) {
            Some(&i) => stored[i].transcript_bytes = found.transcript_bytes,
            None => {
                seen.insert(found.session_id.clone(), stored.len());
                stored.push(found);
            }
        }
    }
    stored.sort_by(|a, b| {
        b.orphaned_at
            .cmp(&a.orphaned_at)
            .then_with(|| a.session_id.cmp(&b.session_id))
    });
    stored
}

/// Where Claude Code keeps its conversations: one directory per working
/// directory, named after it, holding one `<session-id>.jsonl` per session.
fn claude_projects_dir() -> Option<PathBuf> {
    nebula_core::env::home_dir().map(|home| home.join(".claude").join("projects"))
}

/// Claude Code's name for the directory of a given working directory: every
/// character that is not a letter, a digit or a dash becomes a dash, so
/// `D:\repo\wt` becomes `D--repo-wt`.
///
/// The mapping loses information and cannot be reversed — two different
/// checkouts can slug the same. It is used only to narrow which directories
/// are worth opening; what a transcript actually ran in is read back out of
/// the transcript itself.
fn slug(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// The metadata Claude Code writes into the head of a transcript. Every
/// field is optional: the header lines vary by version, and a conversation
/// that never took a turn has no message to carry `cwd` at all.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TranscriptHead {
    cwd: Option<String>,
    git_branch: Option<String>,
    custom_title: Option<String>,
}

impl TranscriptHead {
    fn complete(&self) -> bool {
        self.cwd.is_some() && self.git_branch.is_some() && self.custom_title.is_some()
    }

    /// Take whatever this JSONL line knows and keep the first answer for
    /// each field — the earliest line is the closest to what the session
    /// started as, and a later `cd` must not rewrite where it belongs.
    fn absorb(&mut self, line: &str) {
        #[derive(Deserialize)]
        struct Line {
            #[serde(default)]
            cwd: Option<String>,
            #[serde(default)]
            #[serde(rename = "gitBranch")]
            git_branch: Option<String>,
            #[serde(default)]
            #[serde(rename = "customTitle")]
            custom_title: Option<String>,
        }
        let Ok(parsed) = serde_json::from_str::<Line>(line) else {
            return;
        };
        self.cwd = self.cwd.take().or(parsed.cwd);
        self.git_branch = self.git_branch.take().or(parsed.git_branch);
        self.custom_title = self.custom_title.take().or(parsed.custom_title);
    }
}

/// Read a transcript's head far enough to learn where it ran.
fn read_head(path: &Path) -> Option<TranscriptHead> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut head = TranscriptHead::default();
    let mut spent = 0u64;
    let mut line = String::new();
    while spent < HEAD_BUDGET_BYTES {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(n) => spent += n as u64,
        }
        head.absorb(&line);
        if head.complete() {
            break;
        }
    }
    head.cwd.is_some().then_some(head)
}

/// The two directories a project's conversations can have run in: its own
/// checkout, and the WORKTREE DIR beside it where nebula puts new worktrees
/// (`<repo>/../<repo-name>-worktrees`).
///
/// This is what keeps one project's transcripts out of another's list. The
/// slug prefix alone cannot: `…/nebula` is a prefix of `…/nebula2`, whose
/// conversations would then look orphaned here — and offering to resume
/// them in the wrong repository is worse than missing a worktree somebody
/// created outside both directories.
fn project_dirs(repo: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![repo.to_path_buf()];
    if let (Some(parent), Some(name)) = (repo.parent(), repo.file_name()) {
        dirs.push(parent.join(format!("{}-worktrees", name.to_string_lossy())));
    }
    dirs
}

/// Epoch ms of a file's last write, or 0 when the filesystem won't say.
fn modified_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Claude conversations of `project` that ran in a checkout no longer in
/// its worktree list. Every failure here is a directory that stays unread:
/// this is a best-effort recovery of what the store lost, and it must never
/// take the list down with it.
fn scan_claude_transcripts(
    root: &Path,
    project: &Project,
    live_worktrees: &[Worktree],
) -> Vec<OrphanedSession> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    // A project's conversations live under its own checkout or under the
    // WORKTREE DIR beside it, and both slug to something starting with the
    // repo path's own slug — which narrows the scan cheaply, but only
    // narrows it: `…/nebula` is a prefix of `…/nebula2` as well, so where a
    // transcript really ran is decided by `belongs_to` below.
    let repo = paths::canonical_or_raw(&project.repo_path);
    let prefix = slug(&repo);
    let home = project_dirs(&repo);
    let live: Vec<PathBuf> = live_worktrees
        .iter()
        .map(|w| paths::canonical_or_raw(&w.path))
        .collect();

    let mut found = Vec::new();
    for dir in entries.flatten() {
        if !dir.file_name().to_string_lossy().starts_with(&prefix) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(dir.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(session_id) = path.file_stem().map(|s| s.to_string_lossy().into_owned())
            else {
                continue;
            };
            let Some(head) = read_head(&path) else {
                continue;
            };
            let cwd = paths::canonical_or_raw(Path::new(head.cwd.as_deref().unwrap_or_default()));
            if !home.iter().any(|dir| paths::contains(dir, &cwd)) {
                continue;
            }
            if live.iter().any(|w| paths::contains(w, &cwd)) {
                continue;
            }
            let Ok(meta) = file.metadata() else {
                continue;
            };
            let branch = head.git_branch.unwrap_or_default();
            let name = head
                .custom_title
                .filter(|t| !t.is_empty())
                .or_else(|| (!branch.is_empty()).then(|| branch.clone()))
                .unwrap_or_else(|| session_id.chars().take(8).collect());
            found.push(OrphanedSession {
                session_id,
                project_id: project.id.clone(),
                // The scan reads Claude's store and nothing else, so this is
                // not a guess: anything it finds is a Claude conversation.
                kind: nebula_core::AgentKind::Claude,
                name,
                branch,
                worktree_path: cwd,
                created_at: 0,
                orphaned_at: modified_ms(&meta),
                transcript_bytes: Some(meta.len()),
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slug is a data format owned by another program: nebula only
    /// matches against it, so the mapping has to be the one Claude Code
    /// actually uses, verified against real directory names.
    #[test]
    fn slug_matches_claude_codes_directory_names() {
        assert_eq!(
            slug(Path::new(r"D:\web-projects\nebula")),
            "D--web-projects-nebula"
        );
        assert_eq!(
            slug(Path::new(r"D:\web-projects\nebula-worktrees\features")),
            "D--web-projects-nebula-worktrees-features"
        );
        assert_eq!(slug(Path::new("/home/me/repo")), "-home-me-repo");
        // A leading dot is a character like any other, not a hidden-file rule.
        assert_eq!(
            slug(Path::new(r"D:\web-projects\.hooktest")),
            "D--web-projects--hooktest"
        );
    }

    /// A worktree's slug starts with its project's, which is the whole
    /// reason the prefix filter can narrow the scan without missing rows.
    #[test]
    fn a_worktrees_slug_starts_with_its_projects() {
        let project = slug(Path::new(r"D:\web-projects\nebula"));
        let worktree = slug(Path::new(r"D:\web-projects\nebula-worktrees\features"));
        assert!(worktree.starts_with(&project));
    }

    #[test]
    fn head_reads_cwd_branch_and_title_from_a_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"type":"custom-title","customTitle":"feat-gitlab","sessionId":"s"}"#,
                "\n",
                r#"{"type":"mode","mode":"normal"}"#,
                "\n",
                r#"{"type":"user","cwd":"D:\\web-projects\\wt","gitBranch":"features"}"#,
                "\n",
            ),
        )
        .unwrap();
        let head = read_head(&path).unwrap();
        assert_eq!(head.cwd.as_deref(), Some(r"D:\web-projects\wt"));
        assert_eq!(head.git_branch.as_deref(), Some("features"));
        assert_eq!(head.custom_title.as_deref(), Some("feat-gitlab"));
    }

    /// A transcript with no message in it never reports a working directory,
    /// so there is no way to tell which project it belongs to — it is
    /// skipped rather than filed under a guess.
    #[test]
    fn head_without_a_cwd_is_not_a_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(&path, "{\"type\":\"mode\",\"mode\":\"normal\"}\n").unwrap();
        assert!(read_head(&path).is_none());
    }

    /// Garbage lines are stepped over: a half-written transcript is common
    /// (the CLI appends as it goes) and must not cost the whole file.
    #[test]
    fn head_survives_an_unparsable_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(
            &path,
            concat!(
                "not json at all\n",
                r#"{"cwd":"/w/feat","gitBranch":"feat"}"#,
                "\n",
            ),
        )
        .unwrap();
        let head = read_head(&path).unwrap();
        assert_eq!(head.cwd.as_deref(), Some("/w/feat"));
    }

    /// The first answer wins: a session that `cd`s mid-conversation still
    /// belongs to the checkout it started in.
    #[test]
    fn head_keeps_the_first_cwd_it_sees() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"cwd":"/w/first"}"#,
                "\n",
                r#"{"cwd":"/w/second","gitBranch":"b","customTitle":"t"}"#,
                "\n",
            ),
        )
        .unwrap();
        assert_eq!(read_head(&path).unwrap().cwd.as_deref(), Some("/w/first"));
    }

    /// A transcript, written where Claude Code would write it.
    fn transcript(root: &Path, cwd: &Path, session_id: &str, title: &str, branch: &str) {
        let dir = root.join(slug(cwd));
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = serde_json::to_string(&cwd.to_string_lossy()).unwrap();
        std::fs::write(
            dir.join(format!("{session_id}.jsonl")),
            format!(
                "{{\"type\":\"custom-title\",\"customTitle\":\"{title}\"}}\n\
                 {{\"type\":\"user\",\"cwd\":{cwd},\"gitBranch\":\"{branch}\"}}\n"
            ),
        )
        .unwrap();
    }

    fn project_at(repo: &Path) -> Project {
        Project {
            workspace_id: Default::default(),
            id: nebula_core::ProjectId("p".into()),
            name: "demo".into(),
            repo_path: repo.to_path_buf(),
            sort_order: 0,
        }
    }

    fn worktree_at(path: &Path) -> Worktree {
        Worktree {
            id: nebula_core::WorktreeId("w".into()),
            project_id: nebula_core::ProjectId("p".into()),
            path: path.to_path_buf(),
            branch: "main".into(),
            is_main: true,
            sort_order: 0,
        }
    }

    /// A directory that exists, so its path canonicalizes the same way the
    /// scan will canonicalize it. Windows temp paths in particular do not
    /// compare equal to their canonical form.
    fn dir(path: PathBuf) -> PathBuf {
        std::fs::create_dir_all(&path).unwrap();
        paths::canonical_or_raw(&path)
    }

    /// The rule that decides what "orphaned" means: a conversation whose
    /// checkout is still in the project's worktree list is reachable from
    /// its own session row and must not show up here as well.
    #[test]
    fn a_transcript_in_a_live_worktree_is_not_orphaned() {
        let tmp = tempfile::tempdir().unwrap();
        let base = dir(tmp.path().to_path_buf());
        let repo = dir(base.join("demo"));
        let gone = dir(base.join("demo-worktrees").join("feat"));
        let root = base.join("claude");

        transcript(&root, &repo, "live", "on-main", "main");
        transcript(&root, &gone, "orphan", "feat-gitlab", "feat");

        let found = scan_claude_transcripts(&root, &project_at(&repo), &[worktree_at(&repo)]);

        assert_eq!(found.len(), 1, "only the deleted checkout's session");
        assert_eq!(found[0].session_id, "orphan");
        assert_eq!(found[0].name, "feat-gitlab");
        assert_eq!(found[0].branch, "feat");
        assert_eq!(found[0].kind, nebula_core::AgentKind::Claude);
        assert!(found[0].transcript_bytes.unwrap() > 0);
    }

    /// The scan is scoped by the project's own slug, so another repo's
    /// conversations never leak into this project's list.
    #[test]
    fn another_projects_transcripts_are_not_scanned() {
        let tmp = tempfile::tempdir().unwrap();
        let base = dir(tmp.path().to_path_buf());
        let repo = dir(base.join("demo"));
        let root = base.join("claude");

        transcript(&root, &dir(base.join("elsewhere")), "other", "x", "b");

        assert!(scan_claude_transcripts(&root, &project_at(&repo), &[]).is_empty());
    }

    /// The slug prefix alone would let `demo2`'s conversations through into
    /// `demo`'s list — and resuming one of those would drop a conversation
    /// about a different repository into this one.
    #[test]
    fn a_project_whose_name_extends_this_one_is_not_scanned_into_it() {
        let tmp = tempfile::tempdir().unwrap();
        let base = dir(tmp.path().to_path_buf());
        let repo = dir(base.join("demo"));
        let sibling = dir(base.join("demo2"));
        let root = base.join("claude");

        transcript(&root, &sibling, "next-door", "other-repo", "main");
        assert!(
            slug(&sibling).starts_with(&slug(&repo)),
            "the prefix filter really does let this one through"
        );

        assert!(scan_claude_transcripts(&root, &project_at(&repo), &[]).is_empty());
    }

    /// A transcript with no readable working directory cannot be filed under
    /// any project, so the scan steps over it instead of guessing.
    #[test]
    fn a_transcript_without_a_cwd_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = dir(tmp.path().join("demo"));
        let root = tmp.path().join("claude");
        let dir = root.join(slug(&paths::canonical_or_raw(&repo)));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("s.jsonl"),
            "{\"type\":\"mode\"}
",
        )
        .unwrap();

        assert!(scan_claude_transcripts(&root, &project_at(&repo), &[]).is_empty());
    }

    fn row(session_id: &str, name: &str, orphaned_at: i64) -> OrphanedSession {
        OrphanedSession {
            session_id: session_id.into(),
            project_id: nebula_core::ProjectId("p".into()),
            kind: nebula_core::AgentKind::Codex,
            name: name.into(),
            branch: "feat".into(),
            worktree_path: PathBuf::from("/gone"),
            created_at: 0,
            orphaned_at,
            transcript_bytes: None,
        }
    }

    /// One conversation found in both places is one row: the store's, which
    /// knows the kind and the user's own name for it, plus the one fact only
    /// the transcript has.
    #[test]
    fn a_session_known_to_both_sources_is_one_row() {
        let mut from_disk = row("sid", "claude-title", 50);
        from_disk.kind = nebula_core::AgentKind::Claude;
        from_disk.transcript_bytes = Some(4096);

        let merged = merge(vec![row("sid", "the-name-i-gave-it", 100)], vec![from_disk]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "the-name-i-gave-it", "the store row wins");
        assert_eq!(merged[0].kind, nebula_core::AgentKind::Codex);
        assert_eq!(merged[0].transcript_bytes, Some(4096), "the disk adds this");
    }

    /// Sessions orphaned before the store started keeping them exist only on
    /// disk; the merge is a union, not a filter over what the store knows.
    #[test]
    fn a_session_only_on_disk_still_makes_the_list() {
        let merged = merge(
            vec![row("in-store", "a", 10)],
            vec![row("on-disk", "b", 20)],
        );
        let ids: Vec<&str> = merged.iter().map(|o| o.session_id.as_str()).collect();
        assert_eq!(ids, ["on-disk", "in-store"], "newest first");
    }
}
