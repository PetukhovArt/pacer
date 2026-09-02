//! The GitHub adapter: every question in [`super`]'s interface answered
//! with the GitHub CLI (`gh`), whose `--json` payloads already speak the
//! normal form — `OPEN`/`MERGED`/`CLOSED` states, `isDraft`, review
//! verdicts — so the parsers here mostly relabel.

use std::path::Path;

use super::{arr_at, bool_at, http_url_at, run, str_at, u64_at};
use super::{Approval, Checks, OpenPr, PrComment, PrDetail, PullRequest};
use super::{DIFF_TIMEOUT, LIST_LIMIT, STATE_OPEN, TIMEOUT};

/// Run `gh` with `args` (in `dir` when given), stdout on success.
async fn gh(dir: Option<&Path>, args: &[&str], timeout: std::time::Duration) -> Option<String> {
    run("gh", dir, args, timeout).await
}

/// `v["state"]`, assumed open when `gh` left it out — a PR it lists is
/// open until it says otherwise.
fn state_at(v: &serde_json::Value) -> String {
    v.get("state")
        .and_then(|s| s.as_str())
        .unwrap_or(STATE_OPEN)
        .to_string()
}

/// Ask `gh` for the pull request on `dir`'s current branch. `None` covers
/// every ordinary miss: no PR, no `gh`, no remote, not logged in.
pub(super) async fn lookup(dir: &Path) -> Option<PullRequest> {
    let out = gh(
        Some(dir),
        &[
            "pr",
            "view",
            "--json",
            "number,url,title,state,isDraft,comments,reviews",
        ],
        TIMEOUT,
    )
    .await?;
    // Only asked once `gh` has proved it works, so a machine without it
    // never pays for the extra process.
    parse(&out, viewer_login(dir).await.as_deref())
}

/// Your own GitHub login on the host `dir` talks to. Needed only to keep
/// your own review submissions out of the unread count: `gh` flags comments
/// with `viewerDidAuthor`, but reviews carry nothing but an author — and
/// replying to an inline thread on your own PR files a review.
///
/// Asked *in the checkout* and cached per host — see [`super::Viewers`];
/// a GitHub Enterprise checkout answers with your name there, not with
/// whoever you are on github.com.
static VIEWERS: super::Viewers = super::Viewers::new();

async fn viewer_login(dir: &Path) -> Option<String> {
    VIEWERS
        .resolve(dir, || async {
            let out = gh(Some(dir), &["api", "user", "--jq", ".login"], TIMEOUT).await?;
            Some(out.trim().to_string())
        })
        .await
}

/// Parse `gh pr view --json …` output. Kept separate from the process call
/// so the shape it expects is testable without a GitHub account. `viewer`
/// is your login when it's known; without it your own reviews count as
/// activity, which is a wrong badge rather than a broken one.
fn parse(json: &str, viewer: Option<&str>) -> Option<PullRequest> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let url = http_url_at(&v, "url")?;
    Some(PullRequest {
        number: v.get("number")?.as_u64()?,
        url,
        title: str_at(&v, "title"),
        state: state_at(&v),
        is_draft: bool_at(&v, "isDraft"),
        activity: activity(&v, viewer),
    })
}

/// Timestamps of everything other people posted on the PR — issue comments
/// and review submissions alike, since either is a reason to go look —
/// sorted oldest first so the last one is the high-water mark.
fn activity(v: &serde_json::Value, viewer: Option<&str>) -> Vec<String> {
    let mut stamps: Vec<String> = Vec::new();
    for c in arr_at(v, "comments") {
        if c.get("viewerDidAuthor").and_then(|b| b.as_bool()) == Some(true) {
            continue;
        }
        if let Some(at) = c.get("createdAt").and_then(|t| t.as_str()) {
            stamps.push(at.to_string());
        }
    }
    for r in arr_at(v, "reviews") {
        // No `submittedAt` means a pending review — your own draft, which
        // nobody else can see yet.
        let Some(at) = r.get("submittedAt").and_then(|t| t.as_str()) else {
            continue;
        };
        let author = r
            .get("author")
            .and_then(|a| a.get("login"))
            .and_then(|l| l.as_str());
        if viewer.is_some() && author == viewer {
            continue;
        }
        stamps.push(at.to_string());
    }
    stamps.sort();
    stamps
}

/// Ask `gh` for every open pull request on `dir`'s repo, newest first.
/// The filter rides the same call: `--author @me` for your own, and
/// GitHub's `involves:@me` qualifier — author, assignee, mentioned or
/// commented — for the ones you took part in.
pub(super) async fn list(dir: &Path, filter: super::ListFilter) -> Option<Vec<OpenPr>> {
    let limit = LIST_LIMIT.to_string();
    let mut args = vec![
        "pr",
        "list",
        "--state",
        "open",
        "--limit",
        &limit,
        "--json",
        "number,url,title,isDraft,reviewDecision,statusCheckRollup",
    ];
    match filter {
        super::ListFilter::All => {}
        super::ListFilter::Mine => args.extend(["--author", "@me"]),
        super::ListFilter::Involved => args.extend(["--search", "involves:@me"]),
    }
    let out = gh(Some(dir), &args, TIMEOUT).await?;
    parse_list(&out)
}

/// GitHub's own summary of where the reviewers landed. `null` — the field
/// `gh` emits for a repo that requires no review — is [`Approval::Unknown`]
/// rather than "pending": nobody is being waited on.
fn review_decision(v: &serde_json::Value) -> Approval {
    match str_at(v, "reviewDecision").as_str() {
        "APPROVED" => Approval::Approved,
        "CHANGES_REQUESTED" => Approval::ChangesRequested,
        "REVIEW_REQUIRED" => Approval::Pending,
        _ => Approval::Unknown,
    }
}

/// The one word a whole `statusCheckRollup` array adds up to — the worst
/// entry in it, per [`Checks`]'s ordering.
///
/// GitHub reports the head commit's checks individually and in two shapes:
/// a `CheckRun` carries a `status` and, once it is `COMPLETED`, the
/// `conclusion` that actually judges it; a legacy `StatusContext` carries a
/// bare `state`. A word neither vocabulary defines contributes nothing, so
/// a new GitHub state can only ever leave the glyph quieter — never wrong.
fn checks(v: &serde_json::Value) -> Checks {
    super::arr_at(v, "statusCheckRollup")
        .iter()
        .map(|c| match c.get("status").and_then(|s| s.as_str()) {
            Some("COMPLETED") => conclusion(&str_at(c, "conclusion")),
            Some(status) => conclusion(status),
            None => conclusion(&str_at(c, "state")),
        })
        .max()
        .unwrap_or(Checks::None)
}

/// One check's verdict word as a [`Checks`]. `NEUTRAL` and `SKIPPED` count
/// as passes: they are how a workflow says "not my business", and a row
/// full of them is not a row with a problem. `CANCELLED` does not — the
/// check was supposed to run and never reached a verdict.
fn conclusion(word: &str) -> Checks {
    match word {
        "SUCCESS" | "NEUTRAL" | "SKIPPED" => Checks::Passed,
        "FAILURE" | "ERROR" | "TIMED_OUT" | "STARTUP_FAILURE" | "ACTION_REQUIRED" | "CANCELLED" => {
            Checks::Failed
        }
        "QUEUED" | "IN_PROGRESS" | "WAITING" | "PENDING" | "REQUESTED" | "EXPECTED" => {
            Checks::Running
        }
        _ => Checks::None,
    }
}

/// Parse `gh pr list --json …` output — a bare array. Kept separate from
/// the process call so the shape it expects is testable without a GitHub
/// account. A row whose url could never be opened is dropped rather than
/// failing the whole list; a payload that isn't an array at all is a miss.
fn parse_list(json: &str) -> Option<Vec<OpenPr>> {
    let rows = serde_json::from_str::<serde_json::Value>(json).ok()?;
    let rows = rows.as_array()?;
    Some(
        rows.iter()
            .filter_map(|v| {
                let url = http_url_at(v, "url")?;
                Some(OpenPr {
                    number: v.get("number")?.as_u64()?,
                    title: str_at(v, "title"),
                    url,
                    is_draft: bool_at(v, "isDraft"),
                    approval: review_decision(v),
                    checks: checks(v),
                })
            })
            .collect(),
    )
}

/// Ask `gh` for one pull request's description and conversation.
pub(super) async fn detail(dir: &Path, number: u64) -> Option<PrDetail> {
    let number = number.to_string();
    let out = gh(
        Some(dir),
        &[
            "pr",
            "view",
            &number,
            "--json",
            "number,url,title,state,isDraft,author,baseRefName,headRefName,\
             additions,deletions,changedFiles,body,comments,reviews",
        ],
        TIMEOUT,
    )
    .await?;
    parse_detail(&out)
}

fn parse_detail(json: &str) -> Option<PrDetail> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    Some(PrDetail {
        number: v.get("number")?.as_u64()?,
        url: v.get("url")?.as_str()?.to_string(),
        title: str_at(&v, "title"),
        state: state_at(&v),
        is_draft: bool_at(&v, "isDraft"),
        author: login(v.get("author")),
        base: str_at(&v, "baseRefName"),
        head: str_at(&v, "headRefName"),
        additions: u64_at(&v, "additions"),
        deletions: u64_at(&v, "deletions"),
        changed_files: u64_at(&v, "changedFiles"),
        body: str_at(&v, "body"),
        comments: conversation(&v),
    })
}

fn login(author: Option<&serde_json::Value>) -> String {
    author
        .and_then(|a| a.get("login"))
        .and_then(|l| l.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Comments and review submissions merged into one oldest-first thread.
/// A review with no body is a bare verdict (an approval with nothing typed);
/// it is kept, because "someone approved this" is worth reading, and its
/// empty body renders as the badge alone. Unsubmitted reviews — your own
/// pending draft — are left out; nobody else can see them.
fn conversation(v: &serde_json::Value) -> Vec<PrComment> {
    let mut out: Vec<PrComment> = Vec::new();
    for c in arr_at(v, "comments") {
        out.push(PrComment {
            author: login(c.get("author")),
            at: str_at(c, "createdAt"),
            review_state: String::new(),
            body: str_at(c, "body"),
            ..Default::default()
        });
    }
    for r in arr_at(v, "reviews") {
        let Some(at) = r.get("submittedAt").and_then(|t| t.as_str()) else {
            continue;
        };
        out.push(PrComment {
            author: login(r.get("author")),
            at: at.to_string(),
            review_state: str_at(r, "state"),
            body: str_at(r, "body"),
            ..Default::default()
        });
    }
    // RFC 3339 UTC stamps sort lexicographically into chronological order —
    // the same trick the unread badge uses, so still no date parsing.
    out.sort_by(|a, b| a.at.cmp(&b.at));
    out
}

/// The whole unified diff of a pull request, in one call. `None` when `gh`
/// couldn't answer. `gh pr diff` already emits the `diff --git` per-file
/// shape the splitter expects.
pub(super) async fn diff(dir: &Path, number: u64) -> Option<String> {
    gh(
        Some(dir),
        &["pr", "diff", &number.to_string()],
        DIFF_TIMEOUT,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_gh_pr_view_payload() {
        let pr = parse(
            r#"{"isDraft":false,"number":42,"state":"OPEN","title":"Attach links to worktrees","url":"https://github.com/o/r/pull/42"}"#,
            None,
        )
        .expect("parsed");
        assert_eq!(pr.number, 42);
        assert_eq!(pr.url, "https://github.com/o/r/pull/42");
        assert_eq!(pr.title, "Attach links to worktrees");
        assert_eq!(pr.badge(), "pr");
        assert!(pr.is_open());
    }

    #[test]
    fn refuses_payloads_that_are_not_http_links() {
        // No PR at all, and a payload whose url could never be opened.
        assert!(parse("", None).is_none());
        assert!(parse("{}", None).is_none());
        assert!(parse(r#"{"number":1,"url":"file:///etc/passwd"}"#, None).is_none());
    }

    /// Comments and review submissions both count, both are sorted into one
    /// oldest-first list, and anything the viewer wrote is left out — the
    /// badge is about what *other* people said.
    #[test]
    fn activity_gathers_other_peoples_comments_and_reviews() {
        let pr = parse(
            r#"{
              "number": 42, "url": "https://github.com/o/r/pull/42",
              "comments": [
                {"createdAt": "2024-04-26T21:44:55Z", "viewerDidAuthor": false},
                {"createdAt": "2024-04-27T09:00:00Z", "viewerDidAuthor": true}
              ],
              "reviews": [
                {"submittedAt": "2024-04-25T19:55:42Z", "author": {"login": "steiza"}},
                {"submittedAt": "2024-04-28T08:00:00Z", "author": {"login": "me"}},
                {"author": {"login": "steiza"}}
              ]
            }"#,
            Some("me"),
        )
        .expect("parsed");
        assert_eq!(
            pr.activity,
            ["2024-04-25T19:55:42Z", "2024-04-26T21:44:55Z"],
            "own comment, own review and an unsubmitted review all drop out"
        );
    }

    /// A payload from an older `gh` (or a PR with an empty conversation)
    /// carries no comment arrays at all; that's zero activity, not a miss.
    #[test]
    fn a_payload_without_conversation_fields_still_parses() {
        let pr = parse(
            r#"{"number":1,"url":"https://github.com/o/r/pull/1"}"#,
            Some("me"),
        )
        .expect("parsed");
        assert!(pr.activity.is_empty());
        assert_eq!(pr.seen_marker(), "");
    }

    #[test]
    fn parses_a_gh_pr_list_payload() {
        let prs = parse_list(
            r#"[
              {"number":42,"title":"Attach links","url":"https://github.com/o/r/pull/42","isDraft":false},
              {"number":7,"title":"WIP","url":"https://github.com/o/r/pull/7","isDraft":true}
            ]"#,
        )
        .expect("parsed");
        assert_eq!(prs.len(), 2);
        assert_eq!(prs[0].label(), "#42 Attach links");
        assert_eq!(prs[0].badge(), "pr");
        assert_eq!(prs[1].badge(), "draft");
        // A payload from before these fields were asked for says nothing
        // about either status rather than guessing at one.
        assert_eq!(prs[0].approval, Approval::Unknown);
        assert_eq!(prs[0].checks, Checks::None);
    }

    /// The two status glyphs come out of the same list call as the rows:
    /// `reviewDecision` verbatim, and the check rollup boiled down to its
    /// worst entry.
    #[test]
    fn list_rows_carry_their_review_and_check_status() {
        let prs = parse_list(
            r#"[
              {"number":1,"url":"https://github.com/o/r/pull/1",
               "reviewDecision":"APPROVED",
               "statusCheckRollup":[
                 {"status":"COMPLETED","conclusion":"SUCCESS"},
                 {"status":"COMPLETED","conclusion":"SKIPPED"}
               ]},
              {"number":2,"url":"https://github.com/o/r/pull/2",
               "reviewDecision":"CHANGES_REQUESTED",
               "statusCheckRollup":[
                 {"status":"COMPLETED","conclusion":"SUCCESS"},
                 {"status":"IN_PROGRESS"},
                 {"status":"COMPLETED","conclusion":"FAILURE"}
               ]},
              {"number":3,"url":"https://github.com/o/r/pull/3",
               "reviewDecision":"REVIEW_REQUIRED",
               "statusCheckRollup":[
                 {"status":"COMPLETED","conclusion":"SUCCESS"},
                 {"status":"QUEUED"}
               ]},
              {"number":4,"url":"https://github.com/o/r/pull/4",
               "reviewDecision":null,
               "statusCheckRollup":[{"state":"SUCCESS"}]}
            ]"#,
        )
        .expect("parsed");
        let got: Vec<_> = prs.iter().map(|p| (p.approval, p.checks)).collect();
        assert_eq!(
            got,
            [
                (Approval::Approved, Checks::Passed),
                (Approval::ChangesRequested, Checks::Failed),
                (Approval::Pending, Checks::Running),
                // A repo that requires no review is not a repo waiting on
                // one — and a legacy StatusContext still counts.
                (Approval::Unknown, Checks::Passed),
            ]
        );
    }

    /// One red check outranks any number of green ones, and something in
    /// flight outranks a pass but not a failure. A word from neither
    /// vocabulary contributes nothing at all.
    #[test]
    fn the_check_rollup_reports_its_worst_entry() {
        let rollup = |json: &str| {
            checks(&serde_json::from_str(&format!(r#"{{"statusCheckRollup":{json}}}"#)).unwrap())
        };
        assert_eq!(rollup("[]"), Checks::None, "nothing ran");
        assert_eq!(
            rollup(r#"[{"status":"COMPLETED","conclusion":"SUCCESS"}]"#),
            Checks::Passed
        );
        assert_eq!(
            rollup(
                r#"[{"status":"COMPLETED","conclusion":"SUCCESS"},
                    {"status":"COMPLETED","conclusion":"TIMED_OUT"},
                    {"status":"IN_PROGRESS"}]"#
            ),
            Checks::Failed
        );
        assert_eq!(
            rollup(r#"[{"status":"COMPLETED","conclusion":"NEUTRAL"},{"status":"WAITING"}]"#),
            Checks::Running
        );
        assert_eq!(
            rollup(r#"[{"status":"COMPLETED","conclusion":"WHAT_IS_THIS"}]"#),
            Checks::None,
            "a word we do not know leaves the glyph quiet, never wrong"
        );
    }

    /// An empty repo answers with an empty array — a real answer, not a
    /// miss, so the panel shows "no open pull requests" rather than
    /// pretending it never asked.
    #[test]
    fn an_empty_list_is_an_answer_not_a_miss() {
        assert_eq!(parse_list("[]"), Some(vec![]));
    }

    /// One unusable row must not cost the whole list; a payload that isn't
    /// a list at all is a miss.
    #[test]
    fn list_rows_that_could_never_be_opened_drop_out() {
        let prs = parse_list(
            r#"[
              {"number":1,"url":"file:///etc/passwd"},
              {"url":"https://github.com/o/r/pull/2"},
              {"number":3,"url":"https://github.com/o/r/pull/3"}
            ]"#,
        )
        .expect("parsed");
        assert_eq!(
            prs.len(),
            1,
            "only the row with both a number and an http url"
        );
        assert_eq!(prs[0].label(), "#3", "a missing title still names the PR");
        assert!(parse_list("").is_none());
        assert!(parse_list("{}").is_none());
    }

    /// The preview payload: description, stats, and one merged oldest-first
    /// thread of comments and reviews.
    #[test]
    fn parses_a_gh_pr_view_detail_payload() {
        let d = parse_detail(
            r#"{
              "number": 42, "url": "https://github.com/o/r/pull/42",
              "title": "Attach links", "state": "OPEN", "isDraft": false,
              "author": {"login": "petukhov"},
              "baseRefName": "main", "headRefName": "feat/links",
              "additions": 106, "deletions": 4, "changedFiles": 2,
              "body": "Closes #1\n\nMakes the row.",
              "comments": [
                {"author": {"login": "steiza"}, "createdAt": "2024-04-26T21:44:55Z", "body": "nice"}
              ],
              "reviews": [
                {"author": {"login": "kate"}, "submittedAt": "2024-04-25T19:55:42Z",
                 "state": "APPROVED", "body": "ship it"},
                {"author": {"login": "kate"}, "state": "PENDING", "body": "draft"}
              ]
            }"#,
        )
        .expect("parsed");
        assert_eq!(d.author, "petukhov");
        assert_eq!((d.base.as_str(), d.head.as_str()), ("main", "feat/links"));
        assert_eq!((d.additions, d.deletions, d.changed_files), (106, 4, 2));
        assert!(d.body.starts_with("Closes #1"));
        // The review is older than the comment, so it leads — and the
        // unsubmitted one never shows up.
        assert_eq!(d.comments.len(), 2);
        assert_eq!(d.comments[0].author, "kate");
        assert_eq!(d.comments[0].verdict(), Some("approved"));
        assert_eq!(d.comments[1].author, "steiza");
        assert_eq!(d.comments[1].verdict(), None, "a plain comment has none");
    }

    /// Missing optional fields are zeros and empty strings, not a failed
    /// parse: only the number and url are load-bearing.
    #[test]
    fn a_sparse_detail_payload_still_parses() {
        let d = parse_detail(r#"{"number":1,"url":"https://x.dev/pull/1"}"#).expect("parsed");
        assert_eq!(d.title, "");
        assert_eq!(d.state, "OPEN");
        assert!(d.comments.is_empty());
        assert!(parse_detail("{}").is_none());
    }
}
