//! The GitLab adapter: [`super`]'s questions answered with the GitLab CLI
//! (`glab`), whose `--output json` payloads are raw GitLab REST objects —
//! so unlike the GitHub side, everything here is translation. A merge
//! request becomes a [`PullRequest`]: `iid` is the number, `web_url` the
//! link, `opened`/`merged`/`closed` map onto the normal-form states, and
//! the conversation comes from *notes*, where GitLab mixes what people
//! said with `system: true` bookkeeping ("added 1 commit") that would
//! drown the thread — those are dropped, except the approval note, which
//! is GitLab's spelling of an APPROVED review.
//!
//! One shape difference reaches the diff: `glab mr diff` emits bare
//! `---`/`+++` file headers with no `diff --git` line, so [`diff`]
//! synthesizes those before handing the text to the shared splitter.

use std::path::Path;

use super::{arr_at, bool_at, http_url_at, run, str_at};
use super::{OpenPr, PrComment, PrDetail, PullRequest};
use super::{DIFF_TIMEOUT, LIST_LIMIT, STATE_OPEN, TIMEOUT};

/// Run `glab` with `args` (in `dir` when given), stdout on success.
async fn glab(dir: Option<&Path>, args: &[&str], timeout: std::time::Duration) -> Option<String> {
    run("glab", dir, args, timeout).await
}

/// GitLab's system note for an approval — the only `system: true` note
/// worth surfacing, because it is a review verdict wearing a note's
/// clothes.
const APPROVAL_NOTE: &str = "approved this merge request";

/// `v["state"]` mapped onto the normal form. GitLab says `opened`,
/// `merged`, `closed` (and `locked`, which still accepts nothing new but
/// is not retired either — it stays open rather than lying "closed").
fn state_at(v: &serde_json::Value) -> String {
    match v.get("state").and_then(|s| s.as_str()) {
        Some("merged") => "MERGED".to_string(),
        Some("closed") => "CLOSED".to_string(),
        _ => STATE_OPEN.to_string(),
    }
}

/// Ask `glab` for the merge request on `dir`'s current branch. `-c` folds
/// the notes into the same payload, so one call carries both the MR and
/// the activity the unread badge counts.
pub(super) async fn lookup(dir: &Path) -> Option<PullRequest> {
    let out = glab(
        Some(dir),
        &["mr", "view", "-c", "--output", "json"],
        TIMEOUT,
    )
    .await?;
    // Only asked once `glab` has proved it works, so a machine without it
    // never pays for the extra process.
    parse(&out, viewer_login().await)
}

/// Your own GitLab username, resolved once per process — the same job as
/// the GitHub viewer: keeping your own notes out of the unread count.
/// GitLab notes carry no `viewerDidAuthor`, so the author comparison is
/// all there is.
static VIEWER: tokio::sync::OnceCell<Option<String>> = tokio::sync::OnceCell::const_new();

async fn viewer_login() -> Option<&'static str> {
    VIEWER
        .get_or_init(|| async {
            let out = glab(None, &["api", "user"], TIMEOUT).await?;
            let v: serde_json::Value = serde_json::from_str(&out).ok()?;
            let login = str_at(&v, "username");
            (!login.is_empty()).then_some(login)
        })
        .await
        .as_deref()
}

/// Parse a `glab mr view -c --output json` payload. Kept separate from the
/// process call so the shape it expects is testable without a GitLab
/// account. `viewer` is your username when it's known; without it your own
/// notes count as activity, which is a wrong badge rather than a broken one.
fn parse(json: &str, viewer: Option<&str>) -> Option<PullRequest> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let url = http_url_at(&v, "web_url")?;
    Some(PullRequest {
        number: v.get("iid")?.as_u64()?,
        url,
        title: str_at(&v, "title"),
        state: state_at(&v),
        is_draft: bool_at(&v, "draft"),
        activity: activity(&v, viewer),
    })
}

/// The author's username of a note or MR, `""` when absent.
fn username(v: &serde_json::Value) -> String {
    v.get("author")
        .and_then(|a| a.get("username"))
        .and_then(|u| u.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Whether a note is somebody's words rather than GitLab's bookkeeping.
/// Approvals pass too: "someone approved this" is a reason to go look.
fn is_conversation(note: &serde_json::Value) -> bool {
    !bool_at(note, "system") || str_at(note, "body") == APPROVAL_NOTE
}

/// Timestamps of every note other people posted — comments and approvals
/// alike — sorted oldest first so the last one is the high-water mark.
/// The stamps are GitLab's RFC 3339 UTC (`…Z`), so they sort
/// lexicographically like the GitHub ones.
fn activity(v: &serde_json::Value, viewer: Option<&str>) -> Vec<String> {
    let mut stamps: Vec<String> = Vec::new();
    for note in arr_at(v, "Notes") {
        if !is_conversation(note) {
            continue;
        }
        if viewer.is_some() && viewer == Some(username(note).as_str()) {
            continue;
        }
        let at = str_at(note, "created_at");
        if !at.is_empty() {
            stamps.push(at);
        }
    }
    stamps.sort();
    stamps
}

/// Ask `glab` for every open merge request on `dir`'s repo, newest first.
/// `glab mr list` filters to opened by default, drafts included — the two
/// properties [`super::list`] documents.
pub(super) async fn list(dir: &Path) -> Option<Vec<OpenPr>> {
    let limit = LIST_LIMIT.to_string();
    let out = glab(
        Some(dir),
        &["mr", "list", "--output", "json", "--per-page", &limit],
        TIMEOUT,
    )
    .await?;
    parse_list(&out)
}

/// Parse `glab mr list --output json` output — a bare array of MR objects.
/// A row whose url could never be opened is dropped rather than failing
/// the whole list; a payload that isn't an array at all is a miss.
fn parse_list(json: &str) -> Option<Vec<OpenPr>> {
    let rows = serde_json::from_str::<serde_json::Value>(json).ok()?;
    let rows = rows.as_array()?;
    Some(
        rows.iter()
            .filter_map(|v| {
                let url = http_url_at(v, "web_url")?;
                Some(OpenPr {
                    number: v.get("iid")?.as_u64()?,
                    title: str_at(v, "title"),
                    url,
                    is_draft: bool_at(v, "draft"),
                })
            })
            .collect(),
    )
}

/// Ask `glab` for one merge request's description and conversation.
pub(super) async fn detail(dir: &Path, number: u64) -> Option<PrDetail> {
    let number = number.to_string();
    let out = glab(
        Some(dir),
        &["mr", "view", &number, "-c", "--output", "json"],
        TIMEOUT,
    )
    .await?;
    parse_detail(&out)
}

fn parse_detail(json: &str) -> Option<PrDetail> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    Some(PrDetail {
        number: v.get("iid")?.as_u64()?,
        url: v.get("web_url")?.as_str()?.to_string(),
        title: str_at(&v, "title"),
        state: state_at(&v),
        is_draft: bool_at(&v, "draft"),
        author: username(&v),
        base: str_at(&v, "target_branch"),
        head: str_at(&v, "source_branch"),
        // GitLab's MR object reports how many files changed but not the
        // line counts; zeros here make the preview show the file count
        // alone rather than a made-up "+0 -0".
        additions: 0,
        deletions: 0,
        changed_files: changes_count(&v),
        body: str_at(&v, "description"),
        comments: conversation(&v),
    })
}

/// `changes_count` is a *string*, and a capped one: a big MR says `"999+"`.
/// The leading digits are the number; the cap marker just drops.
fn changes_count(v: &serde_json::Value) -> u64 {
    let s = str_at(v, "changes_count");
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap_or(0)
}

/// The notes that are conversation, oldest first. An approval note becomes
/// a bare APPROVED verdict — the same shape a GitHub review with no body
/// renders as — so the badge logic downstream needs no forge branch.
fn conversation(v: &serde_json::Value) -> Vec<PrComment> {
    let mut out: Vec<PrComment> = Vec::new();
    for note in arr_at(v, "Notes") {
        if !is_conversation(note) {
            continue;
        }
        let approval = bool_at(note, "system");
        out.push(PrComment {
            author: username(note),
            at: str_at(note, "created_at"),
            review_state: if approval {
                "APPROVED".to_string()
            } else {
                String::new()
            },
            body: if approval {
                String::new()
            } else {
                str_at(note, "body")
            },
        });
    }
    out.sort_by(|a, b| a.at.cmp(&b.at));
    out
}

/// The whole unified diff of a merge request. `glab mr diff` prints bare
/// `---`/`+++` headers per file, so the text is normalized into the
/// `diff --git` shape before anyone splits it.
pub(super) async fn diff(dir: &Path, number: u64) -> Option<String> {
    let out = glab(
        Some(dir),
        &["mr", "diff", &number.to_string(), "--color=never"],
        DIFF_TIMEOUT,
    )
    .await?;
    Some(normalize_diff(&out))
}

/// Insert a `diff --git a/x b/x` line before each file's `---`/`+++`
/// header pair, so [`super::split_unified_diff`] cuts `glab`'s output the
/// same way it cuts `gh`'s. A header is only believed when the full
/// `---` / `+++` / `@@` run follows — a *removed* line whose content
/// happens to start with `-- ` renders as `--- …` inside a hunk, and the
/// three-line shape is what tells the two apart. The path comes from the
/// `+++` side, falling back to the `---` side for a deletion.
fn normalize_diff(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if let Some(old) = line.strip_prefix("--- ") {
            let new = lines.get(i + 1).and_then(|l| l.strip_prefix("+++ "));
            let starts_hunk = lines.get(i + 2).is_some_and(|l| l.starts_with("@@"));
            if let Some(new) = new {
                if starts_hunk {
                    let path = if new == "/dev/null" { old } else { new };
                    let path = strip_ab(path);
                    out.push(format!("diff --git a/{path} b/{path}"));
                }
            }
        }
        out.push((*line).to_string());
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// Drop a `a/` or `b/` prefix when the server happened to emit one.
fn strip_ab(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::super::split_unified_diff;
    use super::*;

    /// The real shape: a `glab mr view -c --output json` payload, trimmed.
    /// GitLab's field names throughout — `iid`, `web_url`, `draft`,
    /// `opened` — and notes under go-gitlab's capital-N `Notes`.
    #[test]
    fn parses_a_glab_mr_view_payload() {
        let pr = parse(
            r#"{
              "iid": 1701, "state": "opened", "draft": false,
              "title": "WC-1882 Хранение профилей",
              "web_url": "http://git.vipaks.local/g/p/-/merge_requests/1701",
              "Notes": [
                {"body": "added 1 commit", "system": true,
                 "author": {"username": "locman.ns"}, "created_at": "2026-08-29T11:37:09.455Z"},
                {"body": "looks fine", "system": false,
                 "author": {"username": "kate"}, "created_at": "2026-08-29T12:00:00.000Z"},
                {"body": "self-note", "system": false,
                 "author": {"username": "me"}, "created_at": "2026-08-29T13:00:00.000Z"},
                {"body": "approved this merge request", "system": true,
                 "author": {"username": "kate"}, "created_at": "2026-08-29T14:00:00.000Z"}
              ]
            }"#,
            Some("me"),
        )
        .expect("parsed");
        assert_eq!(pr.number, 1701);
        assert_eq!(pr.url, "http://git.vipaks.local/g/p/-/merge_requests/1701");
        assert_eq!(pr.badge(), "pr");
        assert!(pr.is_open());
        assert_eq!(
            pr.activity,
            ["2026-08-29T12:00:00.000Z", "2026-08-29T14:00:00.000Z"],
            "system bookkeeping and your own note drop out; an approval counts"
        );
    }

    #[test]
    fn gitlab_states_map_onto_the_normal_form() {
        for (gitlab, badge, open) in [
            ("opened", "pr", true),
            ("merged", "merged", false),
            ("closed", "closed", false),
            ("locked", "pr", true),
        ] {
            let pr = parse(
                &format!(r#"{{"iid":1,"web_url":"https://gitlab.com/g/p/-/merge_requests/1","state":"{gitlab}"}}"#),
                None,
            )
            .expect("parsed");
            assert_eq!(pr.badge(), badge, "{gitlab}");
            assert_eq!(pr.is_open(), open, "{gitlab}");
        }
    }

    #[test]
    fn refuses_payloads_that_are_not_http_links() {
        assert!(parse("", None).is_none());
        assert!(parse("{}", None).is_none());
        assert!(parse(r#"{"iid":1,"web_url":"file:///etc/passwd"}"#, None).is_none());
    }

    #[test]
    fn parses_a_glab_mr_list_payload() {
        let prs = parse_list(
            r#"[
              {"iid":1701,"title":"Профили","web_url":"http://git.vipaks.local/g/p/-/merge_requests/1701","draft":false},
              {"iid":7,"title":"WIP","web_url":"http://git.vipaks.local/g/p/-/merge_requests/7","draft":true},
              {"iid":8,"web_url":"file:///nope"}
            ]"#,
        )
        .expect("parsed");
        assert_eq!(prs.len(), 2, "the unopenable row drops out");
        assert_eq!(prs[0].label(), "#1701 Профили");
        assert_eq!(prs[1].badge(), "draft");
        assert_eq!(parse_list("[]"), Some(vec![]));
        assert!(parse_list("{}").is_none());
    }

    /// The preview payload: GitLab names for the branches and description,
    /// a string `changes_count`, and notes as the conversation.
    #[test]
    fn parses_a_glab_mr_detail_payload() {
        let d = parse_detail(
            r#"{
              "iid": 1701, "web_url": "http://git.vipaks.local/g/p/-/merge_requests/1701",
              "title": "Профили", "state": "opened", "draft": false,
              "author": {"username": "locman.ns"},
              "target_branch": "develop", "source_branch": "feat/profiles",
              "changes_count": "54", "description": "Closes WC-1882",
              "Notes": [
                {"body": "approved this merge request", "system": true,
                 "author": {"username": "kate"}, "created_at": "2026-08-29T14:00:00.000Z"},
                {"body": "nice", "system": false,
                 "author": {"username": "kate"}, "created_at": "2026-08-29T12:00:00.000Z"},
                {"body": "added 1 commit", "system": true,
                 "author": {"username": "locman.ns"}, "created_at": "2026-08-29T11:00:00.000Z"}
              ]
            }"#,
        )
        .expect("parsed");
        assert_eq!(d.author, "locman.ns");
        assert_eq!(
            (d.base.as_str(), d.head.as_str()),
            ("develop", "feat/profiles")
        );
        assert_eq!((d.additions, d.deletions, d.changed_files), (0, 0, 54));
        assert_eq!(d.body, "Closes WC-1882");
        // Oldest first, bookkeeping gone, the approval a bare verdict.
        assert_eq!(d.comments.len(), 2);
        assert_eq!(d.comments[0].body, "nice");
        assert_eq!(d.comments[0].verdict(), None);
        assert_eq!(d.comments[1].verdict(), Some("approved"));
        assert_eq!(d.comments[1].body, "");
    }

    /// A capped count keeps its digits; garbage is zero, not a failure.
    #[test]
    fn a_capped_changes_count_still_counts() {
        for (raw, want) in [("999+", 999), ("54", 54), ("", 0), ("?", 0)] {
            let v: serde_json::Value =
                serde_json::from_str(&format!(r#"{{"changes_count":"{raw}"}}"#)).unwrap();
            assert_eq!(changes_count(&v), want, "{raw}");
        }
    }

    /// `glab mr diff` output — bare headers, no `diff --git` — normalizes
    /// into exactly the shape the shared splitter cuts per file.
    #[test]
    fn a_glab_diff_normalizes_and_splits_per_file() {
        let text = "\
--- docs/adr/0013-profile.md
+++ docs/adr/0013-profile.md
@@ -0,0 +1,2 @@
+# Заголовок
+текст
--- src/old.ts
+++ src/new.ts
@@ -1 +1 @@
-x
+y
";
        let files = split_unified_diff(&normalize_diff(text));
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, "docs/adr/0013-profile.md");
        assert!(files[0].1.contains("+# Заголовок"));
        assert_eq!(files[1].0, "src/new.ts");
    }

    /// A deletion's `+++` is `/dev/null`; the name comes from the `---`
    /// side. And a removed line that itself starts with `-- ` must not be
    /// mistaken for a file header.
    #[test]
    fn deletions_and_tricky_hunk_lines_survive_normalizing() {
        let text = "\
--- gone.sql
+++ /dev/null
@@ -1,2 +0,0 @@
--- comment line removed from sql
-select 1;
";
        let files = split_unified_diff(&normalize_diff(text));
        assert_eq!(files.len(), 1, "the sql comment did not start a new file");
        assert_eq!(files[0].0, "gone.sql");
        assert!(files[0].1.contains("-select 1;"));
    }

    /// Already-normal input (a server that emits `a/`-prefixed headers)
    /// gains nothing twice and loses the prefix in the synthesized header.
    #[test]
    fn ab_prefixes_are_stripped() {
        let text = "\
--- a/src/x.rs
+++ b/src/x.rs
@@ -1 +1 @@
-x
+y
";
        let files = split_unified_diff(&normalize_diff(text));
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "src/x.rs");
    }
}
