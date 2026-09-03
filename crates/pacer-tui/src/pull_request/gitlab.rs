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

use std::collections::HashMap;
use std::path::Path;

use super::{arr_at, bool_at, http_url_at, run, str_at};
use super::{Approval, Checks, OpenPr, PrComment, PrDetail, PullRequest};
use super::{DIFF_TIMEOUT, LIST_LIMIT, STATE_OPEN, TIMEOUT};

/// Run `glab` with `args` (in `dir` when given), stdout on success.
async fn glab(dir: Option<&Path>, args: &[&str], timeout: std::time::Duration) -> Option<String> {
    run("glab", dir, args, timeout).await
}

/// GitLab's system note for an approval — the only `system: true` note
/// worth surfacing, because it is a review verdict wearing a note's
/// clothes.
const APPROVAL_NOTE: &str = "approved this merge request";
/// Its counterpart: a reviewer asking for changes, GitLab's spelling of a
/// CHANGES_REQUESTED review.
const CHANGES_NOTE: &str = "requested changes";

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
    parse(&out, viewer_login(dir).await.as_deref())
}

/// Your own GitLab username on the host `dir` talks to — the same job as
/// the GitHub viewer: keeping your own notes out of the unread count, and
/// naming you to the list filters. GitLab notes carry no `viewerDidAuthor`,
/// so the author comparison is all there is.
///
/// Asked *in the checkout* and cached per host — see [`super::Viewers`],
/// which is where the reasoning lives.
static VIEWERS: super::Viewers = super::Viewers::new();

async fn viewer_login(dir: &Path) -> Option<String> {
    VIEWERS
        .resolve(dir, || async {
            let out = glab(Some(dir), &["api", "user"], TIMEOUT).await?;
            let v: serde_json::Value = serde_json::from_str(&out).ok()?;
            Some(str_at(&v, "username"))
        })
        .await
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
    let body = str_at(note, "body");
    !bool_at(note, "system") || body == APPROVAL_NOTE || body == CHANGES_NOTE
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
///
/// The REST list is the rows; [`statuses`] is the approval and pipeline
/// glyphs, which it does not carry. That second call is strictly an
/// enrichment: it runs against a GraphQL schema older self-hosted GitLabs
/// may not have, so failing it leaves the rows exactly as REST described
/// them, with both glyphs blank.
///
/// The filter needs your username (GitLab's CLI has no `@me`): when it
/// can't be had — not logged in, `glab api user` failed — this is a miss,
/// like every other question `glab` couldn't answer. It used to fall back
/// to the unfiltered list, which is how a filter that never worked looked
/// exactly like a filter that had nothing to hide. GitLab search has no
/// `involves:`, so *involved* is two list calls — authored plus reviewing —
/// merged by `iid`; a merge request you only commented on is missed, which
/// is the closest the REST list gets without a request per row.
pub(super) async fn list(dir: &Path, filter: super::ListFilter) -> Option<Vec<OpenPr>> {
    use super::ListFilter;
    let viewer = match filter {
        ListFilter::All => None,
        ListFilter::Mine | ListFilter::Involved => Some(viewer_login(dir).await?),
    };
    let (mut rows, path) = match (filter, viewer.as_deref()) {
        (ListFilter::Mine, Some(me)) => page(dir, &["--author", me]).await?,
        (ListFilter::Involved, Some(me)) => {
            let (mut rows, mut path) = page(dir, &["--author", me]).await?;
            if let Some((extra, extra_path)) = page(dir, &["--reviewer", me]).await {
                path = path.or(extra_path);
                for row in extra {
                    if rows.iter().all(|r| r.number != row.number) {
                        rows.push(row);
                    }
                }
            }
            // Each page came newest-first; the merge keeps that order.
            rows.sort_by(|a, b| b.number.cmp(&a.number));
            (rows, path)
        }
        _ => page(dir, &[]).await?,
    };
    if let Some(path) = path {
        let known = statuses(dir, &path).await;
        for row in &mut rows {
            if let Some(&(approval, checks)) = known.get(&row.number) {
                row.approval = approval;
                row.checks = checks;
            }
        }
    }
    Some(rows)
}

/// One `glab mr list` page with `extra` filter flags: the parsed rows plus
/// the project path the status enrichment joins on.
async fn page(dir: &Path, extra: &[&str]) -> Option<(Vec<OpenPr>, Option<String>)> {
    let limit = LIST_LIMIT.to_string();
    let mut args = vec!["mr", "list", "--output", "json", "--per-page", &limit];
    args.extend_from_slice(extra);
    let out = glab(Some(dir), &args, TIMEOUT).await?;
    let rows = parse_list(&out)?;
    let path = full_path(&out);
    Some((rows, path))
}

/// The project's `group/subgroup/project` path, off the first row's
/// `references.full` (`domination/web/web-client!1704`). Taken from the
/// payload rather than parsed out of the git remote because the server
/// wrote it: a GitLab hosted under a URL subpath, or reached by a remote
/// that redirects to a renamed project, still names itself correctly here.
/// An empty list has no path — and nothing to enrich either.
fn full_path(json: &str) -> Option<String> {
    let rows = serde_json::from_str::<serde_json::Value>(json).ok()?;
    let full = rows
        .as_array()?
        .first()?
        .get("references")?
        .get("full")?
        .as_str()?;
    let (path, _) = full.split_once('!')?;
    (!path.is_empty()).then(|| path.to_string())
}

/// Approval and pipeline state for every open merge request on `full_path`,
/// keyed by `iid`, in one GraphQL call.
///
/// GitLab's REST merge-request list reports neither: approvals live behind
/// a per-MR `/approvals` endpoint and the pipeline behind the single-MR
/// view, so getting these over REST would cost a request per row. GraphQL
/// answers for the whole page at once, which is the only shape that fits
/// [`super::list`]'s refresh beat.
///
/// Everything here degrades to an empty map: no `glab`, an old server
/// without these fields, a query that errors. The rows survive; the glyphs
/// just stay blank.
async fn statuses(dir: &Path, full_path: &str) -> HashMap<u64, (Approval, Checks)> {
    let query = status_query();
    let out = glab(
        Some(dir),
        &[
            "api",
            "graphql",
            "-f",
            &format!("fullPath={full_path}"),
            "-f",
            &format!("query={query}"),
        ],
        TIMEOUT,
    )
    .await;
    out.as_deref().map(parse_statuses).unwrap_or_default()
}

/// The query [`statuses`] sends. The project comes in as a variable rather
/// than interpolated into the document, so a group path never has to be
/// escaped into GraphQL syntax.
///
/// Three fields, all of them long-standing: `approved` and `approvalsLeft`
/// on the merge request, `status` on its head pipeline. Nothing younger
/// gets asked for here — one unknown field fails the whole document, and
/// the document is the only thing standing between the rows and their
/// glyphs.
fn status_query() -> String {
    format!(
        "query($fullPath: ID!) {{ project(fullPath: $fullPath) {{ \
           mergeRequests(state: opened, first: {LIST_LIMIT}) {{ nodes {{ \
           iid approved approvalsLeft headPipeline {{ status }} \
           }} }} }} }}"
    )
}

/// Parse the GraphQL payload [`statuses`] asks for. A node missing its
/// `iid` — or a payload that is an `errors` block instead of data — simply
/// contributes nothing, which is the same as never having asked.
fn parse_statuses(json: &str) -> HashMap<u64, (Approval, Checks)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return HashMap::new();
    };
    let nodes = v
        .get("data")
        .and_then(|d| d.get("project"))
        .and_then(|p| p.get("mergeRequests"))
        .map(|m| arr_at(m, "nodes"))
        .unwrap_or_default();
    nodes
        .iter()
        .filter_map(|n| {
            // GraphQL types `iid` as a string; REST types it as a number,
            // and the rows this joins onto came from REST.
            let iid: u64 = str_at(n, "iid").parse().ok()?;
            Some((iid, (approval(n), pipeline(n))))
        })
        .collect()
}

/// Where a merge request stands with its approvers. GitLab has no single
/// "review decision": `approved` is the whole verdict, and `approvalsLeft`
/// distinguishes "still owed one" from "this project asks for none" — the
/// latter has nothing to say, so it gets no glyph.
///
/// There is deliberately no [`Approval::ChangesRequested`] here. GitLab
/// spells that as a per-reviewer `reviewState`, a much younger field than
/// the two above, and asking for it would risk the whole query on a server
/// that predates it — trading both glyphs for one.
fn approval(n: &serde_json::Value) -> Approval {
    if bool_at(n, "approved") {
        return Approval::Approved;
    }
    match n.get("approvalsLeft").and_then(|a| a.as_u64()) {
        Some(left) if left > 0 => Approval::Pending,
        _ => Approval::Unknown,
    }
}

/// The head pipeline's state. No pipeline at all is [`Checks::None`] — the
/// project runs no CI, or none has started for this branch yet.
fn pipeline(n: &serde_json::Value) -> Checks {
    let status = n
        .get("headPipeline")
        .map(|p| str_at(p, "status"))
        .unwrap_or_default();
    match status.as_str() {
        "SUCCESS" => Checks::Passed,
        "FAILED" | "CANCELED" | "CANCELLED" => Checks::Failed,
        "CREATED" | "WAITING_FOR_RESOURCE" | "PREPARING" | "PENDING" | "RUNNING" | "SCHEDULED" => {
            Checks::Running
        }
        // MANUAL and SKIPPED are a pipeline that deliberately did nothing,
        // and anything GitLab adds later is a word we can't judge. Neither
        // is worth a glyph.
        _ => Checks::None,
    }
}

/// Parse `glab mr list --output json` output — a bare array of MR objects.
/// A row whose url could never be opened is dropped rather than failing
/// the whole list; a payload that isn't an array at all is a miss. The
/// status glyphs start blank and are filled in by [`statuses`].
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
                    approval: Approval::Unknown,
                    checks: Checks::None,
                })
            })
            .collect(),
    )
}

/// Ask `glab` for one merge request's description and conversation.
///
/// The notes `-c` folds in are a flat list: GitLab keeps the thread
/// structure on a separate *discussions* resource, so that is fetched
/// too and, when it answers, its threaded notes replace the flat ones.
/// A failure there (old server, permission) leaves the flat conversation
/// — worse to read, but nothing lost.
pub(super) async fn detail(dir: &Path, number: u64) -> Option<PrDetail> {
    let number = number.to_string();
    let out = glab(
        Some(dir),
        &["mr", "view", &number, "-c", "--output", "json"],
        TIMEOUT,
    )
    .await?;
    let mut detail = parse_detail(&out)?;
    let path = format!("projects/:id/merge_requests/{number}/discussions?per_page=100");
    if let Some(threads) = glab(Some(dir), &["api", &path], TIMEOUT)
        .await
        .and_then(|json| parse_discussions(&json))
    {
        detail.comments = threads;
    }
    Some(detail)
}

/// The `/discussions` payload as threaded comments, oldest thread first
/// and each thread's replies in the order they were said. Every note in
/// a discussion carries the discussion's id as its thread, so the preview
/// can rebuild the tree without knowing GitLab's shape. `None` when the
/// payload isn't a discussion list at all.
fn parse_discussions(json: &str) -> Option<Vec<PrComment>> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let discussions = v.as_array()?;
    let mut threads: Vec<Vec<PrComment>> = Vec::new();
    for d in discussions {
        let id = str_at(d, "id");
        let mut notes: Vec<PrComment> = arr_at(d, "notes")
            .iter()
            .filter(|n| is_conversation(n))
            .map(|n| PrComment {
                thread: id.clone(),
                path: position_at(n),
                resolved: n.get("resolved").and_then(|r| r.as_bool()),
                ..note_comment(n)
            })
            .collect();
        if notes.is_empty() {
            continue;
        }
        notes.sort_by(|a, b| a.at.cmp(&b.at));
        // A lone note is not a thread, just a comment: no tree glyphs.
        if notes.len() == 1 {
            notes[0].thread = String::new();
        }
        threads.push(notes);
    }
    threads.sort_by(|a, b| a[0].at.cmp(&b[0].at));
    Some(threads.into_iter().flatten().collect())
}

/// One note as a comment: an approval or a changes-request system note
/// becomes a bare verdict, anything else keeps its body.
fn note_comment(note: &serde_json::Value) -> PrComment {
    let system = bool_at(note, "system");
    let body = str_at(note, "body");
    let review_state = match (system, body.as_str()) {
        (true, APPROVAL_NOTE) => "APPROVED",
        (true, CHANGES_NOTE) => "CHANGES_REQUESTED",
        _ => "",
    };
    PrComment {
        author: username(note),
        at: str_at(note, "created_at"),
        review_state: review_state.to_string(),
        body: if system { String::new() } else { body },
        ..Default::default()
    }
}

/// `path:line` of a diff note, `path` alone for a whole-file one, empty
/// for a note on the merge request itself.
fn position_at(note: &serde_json::Value) -> String {
    let Some(pos) = note.get("position") else {
        return String::new();
    };
    let path = str_at(pos, "new_path");
    let path = if path.is_empty() {
        str_at(pos, "old_path")
    } else {
        path
    };
    if path.is_empty() {
        return String::new();
    }
    match pos
        .get("new_line")
        .or_else(|| pos.get("old_line"))
        .and_then(|l| l.as_u64())
    {
        Some(line) => format!("{path}:{line}"),
        None => path,
    }
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
        if is_conversation(note) {
            out.push(note_comment(note));
        }
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

    /// Live check against a real checkout — ignored by default, because it
    /// needs `glab` logged into whatever host that repo talks to. It is the
    /// only place the *wiring* is testable: which host `glab` resolves and
    /// whether the answer reaches the filter are both facts about the
    /// process, not about a payload.
    ///
    /// ```text
    /// PACER_GITLAB_TEST_REPO=<a GitLab checkout> \
    ///   cargo test -p pacer-tui --lib gitlab::tests::mine -- --ignored --nocapture
    /// ```
    ///
    /// Without the variable it skips rather than fails: `--ignored` is also
    /// how someone runs *every* slow test, and a machine with no GitLab
    /// checkout has nothing to say about this one.
    #[tokio::test]
    #[ignore = "needs a glab login for the repo named by PACER_GITLAB_TEST_REPO"]
    async fn mine_narrows_the_list_to_your_own_merge_requests() {
        let Ok(dir) = std::env::var("PACER_GITLAB_TEST_REPO") else {
            println!("skipped: set PACER_GITLAB_TEST_REPO to a GitLab checkout");
            return;
        };
        let dir = Path::new(&dir);
        use crate::pull_request::ListFilter;
        let all = super::list(dir, ListFilter::All).await.expect("list all");
        let mine = super::list(dir, ListFilter::Mine).await.expect("list mine");
        println!("all={} mine={}", all.len(), mine.len());
        assert!(
            !all.is_empty(),
            "the repo has no open merge requests to filter"
        );
        assert!(
            mine.len() < all.len(),
            "the filter changed nothing: all={} mine={}",
            all.len(),
            mine.len()
        );
        for pr in &mine {
            assert!(
                all.iter().any(|a| a.number == pr.number),
                "mine is a subset"
            );
        }
    }

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

    /// The query is one line of valid GraphQL with balanced braces and the
    /// project as a variable — the shape `glab api graphql` accepts.
    #[test]
    fn the_status_query_is_one_balanced_line() {
        let q = status_query();
        assert!(!q.contains('\n'), "one line: {q}");
        assert!(q.starts_with("query($fullPath: ID!) {"), "{q}");
        assert!(
            q.contains("mergeRequests(state: opened, first: 100)"),
            "{q}"
        );
        assert!(
            q.contains("iid approved approvalsLeft headPipeline { status }"),
            "{q}"
        );
        assert_eq!(
            q.matches('{').count(),
            q.matches('}').count(),
            "balanced: {q}"
        );
    }

    /// The GraphQL half of the list: approvals and the head pipeline,
    /// keyed by the same `iid` the REST rows carry — as a string on this
    /// side, a number on that one.
    #[test]
    fn statuses_join_onto_the_rest_rows_by_iid() {
        let got = parse_statuses(
            r#"{"data":{"project":{"mergeRequests":{"nodes":[
              {"iid":"1704","approved":false,"approvalsLeft":1,
               "headPipeline":{"status":"FAILED"}},
              {"iid":"1702","approved":true,"approvalsLeft":0,
               "headPipeline":{"status":"SUCCESS"}},
              {"iid":"1701","approved":false,"approvalsLeft":null,
               "headPipeline":{"status":"RUNNING"}},
              {"iid":"1700","approved":false,"approvalsLeft":2,
               "headPipeline":null},
              {"approved":true}
            ]}}}}"#,
        );
        assert_eq!(got.len(), 4, "the node with no iid joins onto nothing");
        assert_eq!(got[&1704], (Approval::Pending, Checks::Failed));
        assert_eq!(got[&1702], (Approval::Approved, Checks::Passed));
        assert_eq!(
            got[&1701],
            (Approval::Unknown, Checks::Running),
            "a project with no approval rules is not a project waiting"
        );
        assert_eq!(
            got[&1700],
            (Approval::Pending, Checks::None),
            "no pipeline ran"
        );
    }

    /// Everything that isn't a data payload leaves the glyphs blank rather
    /// than failing the list the rows came from.
    #[test]
    fn an_unanswerable_status_query_costs_nothing() {
        for json in [
            "",
            "{}",
            r#"{"errors":[{"message":"Field 'approved' doesn't exist"}]}"#,
            r#"{"data":{"project":null}}"#,
        ] {
            assert!(parse_statuses(json).is_empty(), "{json}");
        }
    }

    /// The project path the GraphQL call needs is the server's own, off
    /// the first row's `references.full`.
    #[test]
    fn the_project_path_comes_out_of_the_rest_payload() {
        let path = full_path(
            r#"[{"iid":1704,"references":{"short":"!1704","full":"domination/web/web-client!1704"}}]"#,
        );
        assert_eq!(path.as_deref(), Some("domination/web/web-client"));
        assert_eq!(full_path("[]"), None, "nothing listed, nothing to enrich");
        assert_eq!(full_path(r#"[{"iid":1}]"#), None);
        assert_eq!(full_path("nonsense"), None);
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

    /// The discussions payload threads replies under their root, keeps
    /// bookkeeping out, and reports where on the diff a thread hangs.
    #[test]
    fn discussions_become_threads_with_their_place_on_the_diff() {
        let got = parse_discussions(
            r#"[
              {"id": "aaa", "individual_note": true, "notes": [
                {"id": 1, "body": "changed the description", "system": true,
                 "author": {"username": "bob"}, "created_at": "2026-09-01T05:00:00.000Z"}]},
              {"id": "bbb", "individual_note": false, "notes": [
                {"id": 2, "body": "leaks", "system": false, "resolved": true,
                 "author": {"username": "kate"}, "created_at": "2026-09-01T06:00:00.000Z",
                 "position": {"new_path": "src/a.ts", "position_type": "file"}},
                {"id": 3, "body": "added 1 commit", "system": true,
                 "author": {"username": "bob"}, "created_at": "2026-09-01T06:30:00.000Z"},
                {"id": 4, "body": "moved it", "system": false, "resolved": true,
                 "author": {"username": "bob"}, "created_at": "2026-09-01T07:00:00.000Z",
                 "position": {"new_path": "src/a.ts", "position_type": "file"}}]},
              {"id": "ccc", "individual_note": false, "notes": [
                {"id": 5, "body": "nit", "system": false, "resolved": false,
                 "author": {"username": "kate"}, "created_at": "2026-09-01T05:30:00.000Z",
                 "position": {"new_path": "src/b.ts", "new_line": 58}}]},
              {"id": "ddd", "individual_note": true, "notes": [
                {"id": 6, "body": "requested changes", "system": true,
                 "author": {"username": "kate"}, "created_at": "2026-09-01T08:00:00.000Z"}]}
            ]"#,
        )
        .expect("a list");
        let rows: Vec<(&str, &str, &str, Option<bool>)> = got
            .iter()
            .map(|c| {
                (
                    c.author.as_str(),
                    c.thread.as_str(),
                    c.path.as_str(),
                    c.resolved,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("kate", "", "src/b.ts:58", Some(false)),
                ("kate", "bbb", "src/a.ts", Some(true)),
                ("bob", "bbb", "src/a.ts", Some(true)),
                ("kate", "", "", None),
            ]
        );
        assert_eq!(got[3].verdict(), Some("changes requested"));
        assert!(parse_discussions("{}").is_none(), "not a list");
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
