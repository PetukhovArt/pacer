//! The pull request open on a worktree's branch, discovered with the
//! forge's own CLI — `gh` for GitHub, `glab` for GitLab (where a "pull
//! request" is a merge request; this module keeps GitHub's word and
//! GitHub's state strings as the normal form). The PR itself is never
//! persisted — the row it feeds sits above the worktree's saved links and
//! refreshes on its own, so a PR opened outside nebula shows up without
//! anyone typing its URL. The one thing that outlives the process is how
//! far the user has read into the conversation, which the daemon keeps
//! (`pr_seen`) so the row can say how many comments landed while they were
//! away.
//!
//! Which forge a checkout talks to is read off its git remote (see
//! [`forge`]); every public function here detects and dispatches, so
//! callers never learn there are two backends. The CLI may be missing,
//! unauthenticated, or pointed at a repo with no remote; every one of
//! those is an ordinary "no PR" answer, not an error worth a flash.
//! Lookups are async because they hit the network.

use std::path::Path;

mod forge;
mod github;
mod gitlab;

/// How long a lookup may run before we give up on it. The CLIs retry and
/// can hang on a stalled network; the row is a convenience, not worth a
/// task that never ends.
pub(crate) const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// How long a diff fetch may run. Diffs are bigger than metadata and a
/// forge can be slow to assemble one for a large pull request, so this is
/// looser than [`TIMEOUT`] — but still bounded, because the user is
/// sitting in front of a "loading" flash while it runs.
pub(crate) const DIFF_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

/// The normalized state string for a pull request that still accepts work —
/// the one state both PR shapes and the preview key their badges on.
/// GitLab's `opened` is mapped onto this before anyone downstream sees it.
pub const STATE_OPEN: &str = "OPEN";

/// Whether a state string is [`STATE_OPEN`]; drafts are open too, so this
/// alone never says anything about `is_draft`.
fn state_is_open(state: &str) -> bool {
    state == STATE_OPEN
}

/// Run `bin` with `args` (in `dir` when given) under `timeout`, yielding
/// stdout on success. Every failure — no such binary, bad exit, timeout —
/// is `None`, since each is an ordinary "couldn't ask" to every caller.
pub(crate) async fn run(
    bin: &str,
    dir: Option<&Path>,
    args: &[&str],
    timeout: std::time::Duration,
) -> Option<String> {
    use nebula_core::spawn::NoWindow;
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(args).stdin(std::process::Stdio::null());
    cmd.no_window();
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    let out = tokio::time::timeout(timeout, cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `v[key]` as a string, `""` when absent or not a string.
pub(crate) fn str_at(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string()
}

/// `v[key]` as a number, 0 when absent or not one.
pub(crate) fn u64_at(v: &serde_json::Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

/// `v[key]` as a flag, false when absent or not one.
pub(crate) fn bool_at(v: &serde_json::Value, key: &str) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(false)
}

/// `v[key]` as an array, empty when absent or not one.
pub(crate) fn arr_at<'a>(v: &'a serde_json::Value, key: &str) -> &'a [serde_json::Value] {
    v.get(key)
        .and_then(|c| c.as_array())
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// `v[key]`, but only when it is something a browser can open. Only
/// http(s) reaches `open(1)`; the CLIs have no business returning anything
/// else, but the row leads straight to a browser so it's checked anyway.
pub(crate) fn http_url_at(v: &serde_json::Value, key: &str) -> Option<String> {
    let url = v.get(key)?.as_str()?.to_string();
    (url.starts_with("https://") || url.starts_with("http://")).then_some(url)
}

/// The pull request the forge reports for a checkout's branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub number: u64,
    pub url: String,
    /// Normalized state string: OPEN, MERGED or CLOSED.
    pub state: String,
    pub title: String,
    pub is_draft: bool,
    /// When somebody *other than you* commented or submitted a review, as
    /// the forge's RFC 3339 stamps, oldest first. Those sort
    /// lexicographically, so "posted since the mark we stored" is a string
    /// compare — nebula never has to parse a date or trust a clock.
    pub activity: Vec<String>,
}

impl PullRequest {
    /// Short state word for the row's trailing badge — the same slot the
    /// agent rows use for their CLI kind.
    pub fn badge(&self) -> &'static str {
        match self.state.as_str() {
            STATE_OPEN if self.is_draft => "draft",
            STATE_OPEN => "pr",
            "MERGED" => "merged",
            "CLOSED" => "closed",
            _ => "pr",
        }
    }

    /// Whether the PR is still open (draft included) — the badge is quiet
    /// for these and loud for the ones that no longer accept work.
    pub fn is_open(&self) -> bool {
        state_is_open(&self.state)
    }

    /// The mark to store when the user opens this PR: everything nebula
    /// currently knows about has been read. Empty when nobody has posted —
    /// which still beats no mark at all, since every real stamp sorts above
    /// it, so the next comment to land counts as new.
    pub fn seen_marker(&self) -> &str {
        self.activity.last().map(String::as_str).unwrap_or("")
    }

    /// How many comments and reviews arrived after `marker`. `None` — a PR
    /// never opened from nebula — leaves the whole conversation unread,
    /// which is the honest answer: the user hasn't looked at any of it.
    pub fn unseen(&self, marker: Option<&str>) -> usize {
        marker.map_or(self.activity.len(), |mark| {
            self.activity.iter().filter(|at| at.as_str() > mark).count()
        })
    }
}

/// Ask the forge for the pull request on `dir`'s current branch. `None`
/// covers every ordinary miss: no PR, no CLI, no remote, not logged in.
pub async fn lookup(dir: &Path) -> Option<PullRequest> {
    match forge::detect(dir).await {
        forge::Forge::GitHub => github::lookup(dir).await,
        forge::Forge::GitLab => gitlab::lookup(dir).await,
    }
}

/// Every open pull request on a project's repo, and what it costs to ask.
///
/// A worktree's own PR ([`lookup`]) is one `view` call per checkout; this
/// is one `list` call per *project*, answering "what's still open here?"
/// for the group at the bottom of the worktrees panel. It deliberately
/// carries no conversation: reading comment counts for a hundred rows would
/// be a request each, so the unread badge stays a per-worktree affair.
/// One page, however many PRs the repo has — and one call, except on
/// GitLab, whose REST list omits approvals and pipelines and so costs a
/// second (see `gitlab::statuses`).
///
/// The CLIs page past their own defaults, so the cap is ours to set: a
/// repo with hundreds of open pull requests would spend several API calls
/// per refresh filling rows nobody scrolls to.
pub const LIST_LIMIT: usize = 100;

/// Which open pull requests the project group lists — the `pr_list_filter`
/// user setting. Applied forge-side where the CLI can say it, so the
/// [`LIST_LIMIT`] page isn't spent on rows the filter would drop anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListFilter {
    /// Every open pull request on the repo.
    #[default]
    All,
    /// Only the ones you authored.
    Mine,
    /// The ones you authored or took part in — commented, reviewed.
    Involved,
}

impl ListFilter {
    /// The filter a config value names. Unknown words are [`Self::All`],
    /// so a hand-edited config can never hide the list by accident.
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "mine" => Self::Mine,
            "involved" => Self::Involved,
            _ => Self::All,
        }
    }
}

/// Where a pull request stands with its reviewers — the left half of the
/// status pair an OPEN PRS row leads with. Deliberately coarse: the row
/// has one cell to say this in, so *who* approved and *how many* are still
/// owed is the preview's job, not the glyph's.
///
/// The order of the variants is the order of alarm, least first — see
/// [`Checks`], which is read the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Approval {
    /// Nothing to say: the forge asks nobody to review this, or it didn't
    /// answer. An empty cell rather than a pending one — a repo with no
    /// review rules must not read as if every row were blocked on someone.
    #[default]
    Unknown,
    Approved,
    /// Somebody still has to look.
    Pending,
    /// A reviewer asked for changes.
    ChangesRequested,
}

/// Where a pull request stands with CI — the right half of the pair.
///
/// A pull request has many checks and one cell to report them in, so the
/// variants are ordered least to most alarming and the row shows the
/// **worst** one: a single red check makes a red row however many green
/// ones surround it, and something still running only outranks a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Checks {
    /// Nothing ran, or the forge didn't answer.
    #[default]
    None,
    Passed,
    /// Queued or in flight.
    Running,
    Failed,
}

/// One row of a project's open-pull-request list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenPr {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub is_draft: bool,
    /// Whether the reviewers have signed off. Both forges answer this in
    /// the same list call the rows come from.
    pub approval: Approval,
    /// Whether CI is happy. Ditto — no extra round trip per row.
    pub checks: Checks,
}

impl OpenPr {
    /// Row text: `#42 title`, the same shape the worktree link rows use.
    pub fn label(&self) -> String {
        if self.title.is_empty() {
            format!("#{}", self.number)
        } else {
            format!("#{} {}", self.number, self.title)
        }
    }

    /// Trailing badge — every row here is open by construction, so the only
    /// thing left to say is whether it's still a draft.
    pub fn badge(&self) -> &'static str {
        if self.is_draft {
            "draft"
        } else {
            "pr"
        }
    }
}

/// Ask the forge for every open pull request on `dir`'s repo, newest first.
/// `None` is "couldn't ask" — no CLI, no remote, not logged in, timed out —
/// and is deliberately distinct from `Some(vec![])`, which is the real
/// answer "nothing is open": the caller keeps the last good list rather than
/// blanking the panel over one failed call.
///
/// Two properties of the open-state filter the group depends on:
///
/// * **Drafts are in it.** A draft *is* an open pull request, and it is the
///   one most likely to have a nebula worktree still attached to it — the
///   list would be worth least if it hid exactly the work in progress. They
///   arrive with the draft flag set and wear a `draft` badge; nothing here
///   or downstream filters them out.
/// * **Closed ones fall out of it.** This is the whole mechanism for
///   pruning: a pull request that was merged or closed since the last call
///   simply stops coming back, so re-asking on a beat *is* the periodic
///   "should this row still be here?" check. Nothing has to track closures
///   separately.
pub async fn list(dir: &Path, filter: ListFilter) -> Option<Vec<OpenPr>> {
    match forge::detect(dir).await {
        forge::Forge::GitHub => github::list(dir, filter).await,
        forge::Forge::GitLab => gitlab::list(dir, filter).await,
    }
}

/// The readable contents of one pull request: what it says it does, and
/// what people said back. Fetched on demand — only for the row the cursor
/// actually rests on — and cached for the session, because this is a second
/// API call on top of the list and the body of a merged-or-not pull request
/// does not change while you read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrDetail {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub author: String,
    /// Branch this merges into, and the branch it comes from.
    pub base: String,
    pub head: String,
    /// Line counts are GitHub-only; GitLab's API reports files but not
    /// lines, so both sit at zero there and the preview shows the file
    /// count alone.
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    /// The description, verbatim markdown. Rendered as plain wrapped text —
    /// nebula is not a markdown viewer, and mangling someone's fenced code
    /// block would be worse than showing it as written.
    pub body: String,
    /// Issue comments and review submissions in one list, oldest first —
    /// the order they were said in, which is the order they read in.
    pub comments: Vec<PrComment>,
}

impl PrDetail {
    /// Whether this pull request still accepts work. A draft counts: it is
    /// open, just not finished. This is the per-row second opinion on the
    /// question [`list`] answers in bulk — when the cursor rests on a row
    /// long enough to fetch its detail, the forge gets asked about that one
    /// pull request directly, and a `MERGED` or `CLOSED` answer retires the
    /// row without waiting for the next list.
    pub fn is_open(&self) -> bool {
        state_is_open(&self.state)
    }
}

/// One thing somebody said on a pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrComment {
    pub author: String,
    /// RFC 3339, as the forge gives it.
    pub at: String,
    /// Empty for a plain comment; the review state (`APPROVED`,
    /// `CHANGES_REQUESTED`, `COMMENTED`) when it came in as a review.
    pub review_state: String,
    pub body: String,
}

impl PrComment {
    /// Short word for the row's badge, or None for a plain comment.
    pub fn verdict(&self) -> Option<&'static str> {
        match self.review_state.as_str() {
            "APPROVED" => Some("approved"),
            "CHANGES_REQUESTED" => Some("changes requested"),
            "DISMISSED" => Some("dismissed"),
            _ => None,
        }
    }
}

/// Ask the forge for one pull request's description and conversation.
/// `number` picks the PR, so this works from any checkout of the repo —
/// the row the cursor is on need not be checked out anywhere.
pub async fn detail(dir: &Path, number: u64) -> Option<PrDetail> {
    match forge::detect(dir).await {
        forge::Forge::GitHub => github::detail(dir, number).await,
        forge::Forge::GitLab => gitlab::detail(dir, number).await,
    }
}

/// The whole unified diff of a pull request, in one call, always in the
/// `diff --git`-per-file shape [`split_unified_diff`] cuts. `None` when
/// the CLI couldn't answer.
pub async fn diff(dir: &Path, number: u64) -> Option<String> {
    match forge::detect(dir).await {
        forge::Forge::GitHub => github::diff(dir, number).await,
        forge::Forge::GitLab => gitlab::diff(dir, number).await,
    }
}

/// Cut a unified diff into one chunk per file, in the order git emitted
/// them: `(path, that file's diff text)`.
///
/// The path comes from the `+++ b/…` line when there is one and falls back
/// to the `diff --git` header, so a deleted file (whose `+++` is
/// `/dev/null`) still reports the path it had. Anything before the first
/// `diff --git` — the CLIs print nothing there today, but a future banner
/// would land there — is dropped rather than shown as a nameless file.
pub fn split_unified_diff(text: &str) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Vec::new();
    let mut path = String::new();
    let mut lines: Vec<&str> = Vec::new();
    let flush = |files: &mut Vec<(String, String)>, path: &mut String, lines: &mut Vec<&str>| {
        if !path.is_empty() {
            files.push((std::mem::take(path), lines.join("\n")));
        }
        lines.clear();
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            flush(&mut files, &mut path, &mut lines);
            path = header_path(rest);
        }
        if path.is_empty() {
            continue; // preamble before the first file
        }
        // `+++ b/x` is authoritative: it survives the quoting and the
        // spaces-in-names ambiguity that makes `diff --git` hard to split.
        if let Some(rest) = line.strip_prefix("+++ ") {
            if rest != "/dev/null" {
                path = rest.strip_prefix("b/").unwrap_or(rest).to_string();
            }
        }
        lines.push(line);
    }
    flush(&mut files, &mut path, &mut lines);
    files
}

/// Best-effort path out of a `diff --git a/x b/x` header. The two halves
/// are the same path for everything but a rename, so the second half is
/// taken and the `b/` prefix stripped; a name containing spaces makes this
/// ambiguous, which is why the `+++` line overrides it when one follows.
fn header_path(rest: &str) -> String {
    let rest = rest.trim();
    match rest.split_once(" b/") {
        Some((_, b)) => b.to_string(),
        None => rest
            .rsplit(' ')
            .next()
            .unwrap_or(rest)
            .strip_prefix("b/")
            .unwrap_or(rest)
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PR carrying `activity`, for the counting tests.
    fn with_activity(stamps: &[&str]) -> PullRequest {
        PullRequest {
            number: 1,
            url: "https://github.com/o/r/pull/1".into(),
            title: "t".into(),
            state: "OPEN".into(),
            is_draft: false,
            activity: stamps.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn badges_name_the_state() {
        let base = PullRequest {
            number: 1,
            url: "https://x.dev/pull/1".into(),
            title: "t".into(),
            state: "OPEN".into(),
            is_draft: true,
            activity: vec![],
        };
        assert_eq!(base.badge(), "draft");
        assert!(base.is_open(), "a draft is still open");
        let merged = PullRequest {
            state: "MERGED".into(),
            is_draft: false,
            ..base.clone()
        };
        assert_eq!(merged.badge(), "merged");
        assert!(!merged.is_open());
        let closed = PullRequest {
            state: "CLOSED".into(),
            ..merged
        };
        assert_eq!(closed.badge(), "closed");
    }

    /// The unread count is a comparison against the mark stored on the last
    /// open — no mark means nothing has been read.
    #[test]
    fn unseen_counts_what_landed_after_the_mark() {
        let pr = with_activity(&[
            "2024-04-25T19:55:42Z",
            "2024-04-26T21:44:55Z",
            "2024-04-27T09:00:00Z",
        ]);
        assert_eq!(pr.unseen(None), 3, "never opened: all of it is unread");
        assert_eq!(pr.unseen(Some("2024-04-25T19:55:42Z")), 2);
        assert_eq!(pr.unseen(Some(pr.seen_marker())), 0, "opening clears it");
    }

    /// Opening a PR nobody has posted on stores an empty mark, and that
    /// mark still does its job: every real timestamp sorts above it, so the
    /// next comment to land reads as new.
    #[test]
    fn an_empty_mark_still_catches_the_next_comment() {
        let quiet = with_activity(&[]);
        assert_eq!(quiet.seen_marker(), "");
        assert_eq!(quiet.unseen(Some("")), 0);

        let later = with_activity(&["2024-04-26T21:44:55Z"]);
        assert_eq!(later.unseen(Some("")), 1);
    }

    /// Deleted comments shrink the list; the count must not go negative or
    /// wrap — it just reports nothing new.
    #[test]
    fn a_deleted_comment_does_not_invent_unread_ones() {
        let pr = with_activity(&["2024-04-25T19:55:42Z"]);
        assert_eq!(pr.unseen(Some("2024-04-27T09:00:00Z")), 0);
    }

    /// The diff is cut per file, in git's order, with the `+++ b/…` line
    /// naming each chunk.
    #[test]
    fn a_unified_diff_splits_per_file() {
        let text = "\
diff --git a/src/a.rs b/src/a.rs
index 111..222 100644
--- a/src/a.rs
+++ b/src/a.rs
@@ -1,2 +1,2 @@
-old
+new
diff --git a/src/b.rs b/src/b.rs
--- a/src/b.rs
+++ b/src/b.rs
@@ -1 +1 @@
-x
+y
";
        let files = split_unified_diff(text);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, "src/a.rs");
        assert!(files[0].1.starts_with("diff --git a/src/a.rs"));
        assert!(files[0].1.contains("+new"));
        assert!(
            !files[0].1.contains("src/b.rs"),
            "the chunk stops at the next file: {}",
            files[0].1
        );
        assert_eq!(files[1].0, "src/b.rs");
        // Nothing is lost and nothing is duplicated: every input line lands
        // in exactly one chunk. Checked against real `gh pr diff` output
        // too, which is what this invariant is really guarding.
        let total: usize = files.iter().map(|(_, d)| d.lines().count()).sum();
        assert_eq!(total, text.lines().count());
    }

    /// A deleted file's `+++` is `/dev/null`, so the name has to come from
    /// the `diff --git` header — and a rename reports the new path.
    #[test]
    fn deleted_and_renamed_files_still_get_a_name() {
        let files = split_unified_diff(
            "\
diff --git a/gone.rs b/gone.rs
deleted file mode 100644
--- a/gone.rs
+++ /dev/null
@@ -1 +0,0 @@
-x
diff --git a/old.rs b/new.rs
similarity index 90%
rename from old.rs
rename to new.rs
--- a/old.rs
+++ b/new.rs
",
        );
        assert_eq!(files[0].0, "gone.rs");
        assert_eq!(files[1].0, "new.rs");
    }

    /// Config words map onto filters; anything else — typos, a config from
    /// the future — shows everything rather than hiding work.
    #[test]
    fn list_filter_names_resolve_with_all_as_the_fallback() {
        assert_eq!(ListFilter::from_name("mine"), ListFilter::Mine);
        assert_eq!(ListFilter::from_name(" Involved "), ListFilter::Involved);
        assert_eq!(ListFilter::from_name("all"), ListFilter::All);
        assert_eq!(ListFilter::from_name(""), ListFilter::All);
        assert_eq!(ListFilter::from_name("nonsense"), ListFilter::All);
    }

    /// Nothing to split is an empty list, not a nameless file.
    #[test]
    fn an_empty_diff_yields_no_files() {
        assert!(split_unified_diff("").is_empty());
        assert!(split_unified_diff("some banner\nwith no diff\n").is_empty());
    }
}
