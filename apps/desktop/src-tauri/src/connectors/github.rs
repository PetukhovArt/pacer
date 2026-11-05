use serde_json::{json, Value};

use super::http::{opt, text, ApiError, ApiResult};
use super::types::{
    ChangeRequest, ChangeRequestSummary, CrState, DiffFile, Discussion, FileStatus, InboxItem, Job,
    Note, Person, Pipeline, PipelineState, Position, Reason,
};
use super::Ctx;

const PAGE: usize = 100;
const MAX_PAGES: usize = 30;

const AUTHOR: &str = "author { login avatarUrl ... on User { name } }";

fn summary_fields() -> String {
    format!(
        "number title url isDraft state updatedAt headRefName baseRefName {AUTHOR} \
         repository {{ nameWithOwner }} \
         reviewThreads(first: 100) {{ nodes {{ isResolved }} }} \
         commits(last: 1) {{ nodes {{ commit {{ statusCheckRollup {{ state }} }} }} }}"
    )
}

fn api_base(ctx: &Ctx) -> String {
    if is_dotcom(ctx) {
        "https://api.github.com".to_owned()
    } else {
        format!("{}/api/v3", ctx.base_url)
    }
}

fn graphql_url(ctx: &Ctx) -> String {
    if is_dotcom(ctx) {
        "https://api.github.com/graphql".to_owned()
    } else {
        format!("{}/api/graphql", ctx.base_url)
    }
}

fn is_dotcom(ctx: &Ctx) -> bool {
    reqwest::Url::parse(&ctx.base_url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host == "github.com"))
        .unwrap_or(false)
}

fn rest_person(user: &Value) -> Person {
    let username = text(&user["login"]);
    if username.is_empty() {
        return Person::ghost();
    }
    Person {
        name: opt(&user["name"]).unwrap_or_else(|| username.clone()),
        username,
        avatar_url: opt(&user["avatar_url"]),
    }
}

fn gql_person(actor: &Value) -> Person {
    let username = opt(&actor["login"]).or_else(|| opt(&actor["slug"]));
    let Some(username) = username else {
        return Person::ghost();
    };
    Person {
        name: opt(&actor["name"]).unwrap_or_else(|| username.clone()),
        username,
        avatar_url: opt(&actor["avatarUrl"]),
    }
}

pub async fn whoami(ctx: &Ctx) -> ApiResult<Person> {
    let user = ctx
        .api
        .json(&format!("{}/user", api_base(ctx)), &[])
        .await?;
    Ok(rest_person(&user))
}

async fn graphql(ctx: &Ctx, query: &str, variables: Value) -> ApiResult<Value> {
    let body = json!({ "query": query, "variables": variables });
    let response = ctx.api.post_json(&graphql_url(ctx), &body).await?;
    if response["data"].is_null() {
        let message = response["errors"][0]["message"]
            .as_str()
            .unwrap_or("пустой ответ GraphQL");
        return Err(ApiError::failed(format!("GitHub GraphQL: {message}")));
    }
    Ok(response["data"].clone())
}

fn split_project(project: &str) -> ApiResult<(&str, &str)> {
    project
        .split_once('/')
        .ok_or_else(|| ApiError::failed(format!("некорректный репозиторий: {project}")))
}

fn rollup(node: &Value) -> PipelineState {
    match node["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["state"].as_str() {
        Some("SUCCESS") => PipelineState::Success,
        Some("FAILURE") | Some("ERROR") => PipelineState::Failed,
        Some("PENDING") | Some("EXPECTED") => PipelineState::Pending,
        _ => PipelineState::None,
    }
}

fn summary(connector: &str, node: &Value) -> Option<ChangeRequestSummary> {
    let number = node["number"].as_u64()?;
    let project = opt(&node["repository"]["nameWithOwner"])?;
    let unresolved = node["reviewThreads"]["nodes"].as_array().map(|threads| {
        threads
            .iter()
            .filter(|thread| thread["isResolved"] == Value::Bool(false))
            .count() as u32
    });
    Some(ChangeRequestSummary {
        connector: connector.to_owned(),
        project,
        number,
        title: text(&node["title"]),
        url: text(&node["url"]),
        state: match node["state"].as_str() {
            Some("MERGED") => CrState::Merged,
            Some("CLOSED") => CrState::Closed,
            _ => CrState::Open,
        },
        draft: node["isDraft"].as_bool().unwrap_or(false),
        author: gql_person(&node["author"]),
        source_branch: text(&node["headRefName"]),
        target_branch: text(&node["baseRefName"]),
        updated_at: text(&node["updatedAt"]),
        pipeline: rollup(node),
        unresolved_threads: unresolved,
    })
}

pub async fn inbox(ctx: &Ctx) -> ApiResult<Vec<InboxItem>> {
    let fields = summary_fields();
    let search = |alias: &str, qualifier: &str| {
        format!(
            "{alias}: search(query: \"is:open is:pr archived:false {qualifier}\", type: ISSUE, first: 50) \
             {{ nodes {{ ... on PullRequest {{ {fields} }} }} }}"
        )
    };
    let query = format!(
        "query {{ {} {} {} }}",
        search("a", "author:@me"),
        search("r", "review-requested:@me"),
        search("s", "assignee:@me")
    );
    let data = graphql(ctx, &query, json!({})).await?;
    let mut items: Vec<InboxItem> = Vec::new();
    for (alias, reason) in [
        ("a", Reason::Author),
        ("r", Reason::Reviewer),
        ("s", Reason::Assignee),
    ] {
        let Some(nodes) = data[alias]["nodes"].as_array() else {
            continue;
        };
        for node in nodes {
            let Some(found) = summary(&ctx.id, node) else {
                continue;
            };
            match items.iter_mut().find(|item| {
                item.summary.project == found.project && item.summary.number == found.number
            }) {
                Some(item) => {
                    if !item.reasons.contains(&reason) {
                        item.reasons.push(reason);
                    }
                }
                None => items.push(InboxItem {
                    summary: found,
                    reasons: vec![reason],
                }),
            }
        }
    }
    Ok(items)
}

pub async fn for_branch(
    ctx: &Ctx,
    project: &str,
    branch: &str,
) -> ApiResult<Option<ChangeRequestSummary>> {
    let (owner, name) = split_project(project)?;
    let query = format!(
        "query($o: String!, $n: String!, $b: String!) {{ repository(owner: $o, name: $n) {{ \
         pullRequests(headRefName: $b, states: OPEN, first: 1, orderBy: {{field: UPDATED_AT, direction: DESC}}) \
         {{ nodes {{ {} }} }} }} }}",
        summary_fields()
    );
    let data = graphql(ctx, &query, json!({ "o": owner, "n": name, "b": branch })).await?;
    Ok(data["repository"]["pullRequests"]["nodes"][0]
        .as_object()
        .and_then(|_| summary(&ctx.id, &data["repository"]["pullRequests"]["nodes"][0])))
}

async fn pull_request(ctx: &Ctx, project: &str, number: u64, fields: &str) -> ApiResult<Value> {
    let (owner, name) = split_project(project)?;
    let query = format!(
        "query($o: String!, $n: String!, $num: Int!) {{ repository(owner: $o, name: $n) {{ \
         pullRequest(number: $num) {{ {fields} }} }} }}"
    );
    let data = graphql(ctx, &query, json!({ "o": owner, "n": name, "num": number })).await?;
    let node = data["repository"]["pullRequest"].clone();
    if node.is_null() {
        return Err(ApiError::failed(format!(
            "PR {project}#{number} не найден на GitHub"
        )));
    }
    Ok(node)
}

pub async fn change_request(ctx: &Ctx, project: &str, number: u64) -> ApiResult<ChangeRequest> {
    let fields = format!(
        "{} body createdAt mergeable labels(first: 30) {{ nodes {{ name }} }} \
         reviewRequests(first: 30) {{ nodes {{ requestedReviewer {{ \
         ... on User {{ login name avatarUrl }} ... on Team {{ slug name avatarUrl }} ... on Bot {{ login avatarUrl }} }} }} }} \
         latestReviews(first: 30) {{ nodes {{ {AUTHOR} }} }}",
        summary_fields()
    );
    let node = pull_request(ctx, project, number, &fields).await?;
    let found = summary(&ctx.id, &node)
        .ok_or_else(|| ApiError::failed("неожиданный ответ GitHub: нет данных PR"))?;
    let mut reviewers: Vec<Person> = Vec::new();
    let requested = node["reviewRequests"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| &entry["requestedReviewer"]);
    let reviewed = node["latestReviews"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| &entry["author"]);
    for actor in requested.chain(reviewed) {
        if actor.is_null() {
            continue;
        }
        let person = gql_person(actor);
        if reviewers
            .iter()
            .all(|known| known.username != person.username)
        {
            reviewers.push(person);
        }
    }
    Ok(ChangeRequest {
        summary: found,
        description: text(&node["body"]),
        reviewers,
        labels: node["labels"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|label| opt(&label["name"]))
            .collect(),
        created_at: text(&node["createdAt"]),
        mergeable: match node["mergeable"].as_str() {
            Some("MERGEABLE") => Some(true),
            Some("CONFLICTING") => Some(false),
            _ => None,
        },
    })
}

pub async fn diff(ctx: &Ctx, project: &str, number: u64) -> ApiResult<Vec<DiffFile>> {
    let url = format!("{}/repos/{project}/pulls/{number}/files", api_base(ctx));
    let mut files = Vec::new();
    for page in 1..=MAX_PAGES {
        let page_text = page.to_string();
        let per_page = PAGE.to_string();
        let batch = ctx
            .api
            .json(&url, &[("per_page", &per_page), ("page", &page_text)])
            .await?;
        let Some(entries) = batch.as_array() else {
            break;
        };
        for entry in entries {
            files.push(diff_file(entry));
        }
        if entries.len() < PAGE {
            break;
        }
    }
    Ok(files)
}

fn diff_file(entry: &Value) -> DiffFile {
    let new_path = text(&entry["filename"]);
    let old_path = opt(&entry["previous_filename"]).unwrap_or_else(|| new_path.clone());
    let status = match entry["status"].as_str() {
        Some("added") | Some("copied") => FileStatus::Added,
        Some("removed") => FileStatus::Deleted,
        Some("renamed") => FileStatus::Renamed,
        _ => FileStatus::Modified,
    };
    let additions = entry["additions"].as_u64().unwrap_or(0) as u32;
    let deletions = entry["deletions"].as_u64().unwrap_or(0) as u32;
    let patch = opt(&entry["patch"]);
    let missing = patch.is_none();
    let changed = additions + deletions > 0;
    DiffFile {
        old_path,
        new_path,
        status,
        patch: patch.unwrap_or_default(),
        additions,
        deletions,
        binary: missing && !changed && status != FileStatus::Renamed,
        too_large: missing && changed,
    }
}

pub async fn discussions(ctx: &Ctx, project: &str, number: u64) -> ApiResult<Vec<Discussion>> {
    let comment = format!("id body createdAt {AUTHOR}");
    let fields = format!(
        "reviewThreads(first: 100) {{ nodes {{ id isResolved isOutdated path line originalLine diffSide \
         comments(first: 100) {{ nodes {{ {comment} }} }} }} }} \
         comments(first: 100) {{ nodes {{ {comment} }} }} \
         reviews(first: 100) {{ nodes {{ {comment} }} }}"
    );
    let node = pull_request(ctx, project, number, &fields).await?;
    let note = |entry: &Value| Note {
        id: text(&entry["id"]),
        author: gql_person(&entry["author"]),
        body: text(&entry["body"]),
        created_at: text(&entry["createdAt"]),
        system: false,
    };
    let mut found: Vec<Discussion> = Vec::new();
    for thread in node["reviewThreads"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let notes: Vec<Note> = thread["comments"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(note)
            .collect();
        if notes.is_empty() {
            continue;
        }
        let line = thread["line"]
            .as_u64()
            .or_else(|| thread["originalLine"].as_u64())
            .map(|line| line as u32);
        let left = thread["diffSide"].as_str() == Some("LEFT");
        found.push(Discussion {
            id: text(&thread["id"]),
            resolvable: true,
            resolved: thread["isResolved"].as_bool().unwrap_or(false),
            position: Some(Position {
                path: text(&thread["path"]),
                new_line: if left { None } else { line },
                old_line: if left { line } else { None },
            }),
            notes,
        });
    }
    let plain = node["comments"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(node["reviews"]["nodes"].as_array().into_iter().flatten())
        .filter(|entry| !text(&entry["body"]).trim().is_empty());
    for entry in plain {
        found.push(Discussion {
            id: text(&entry["id"]),
            resolvable: false,
            resolved: false,
            position: None,
            notes: vec![note(entry)],
        });
    }
    found.sort_by(|a, b| a.notes[0].created_at.cmp(&b.notes[0].created_at));
    Ok(found)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn epoch(stamp: &str) -> Option<i64> {
    let (date, time) = stamp.trim_end_matches('Z').split_once('T')?;
    let mut d = date.split('-').map(|part| part.parse::<i64>());
    let mut t = time.split(':').map(|part| part.parse::<i64>());
    let (year, month, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let (hour, minute, second) = (t.next()?.ok()?, t.next()?.ok()?, t.next()?.ok()?);
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

pub async fn pipeline(ctx: &Ctx, project: &str, number: u64) -> ApiResult<Option<Pipeline>> {
    let base = api_base(ctx);
    let pull = ctx
        .api
        .json(&format!("{base}/repos/{project}/pulls/{number}"), &[])
        .await?;
    let Some(sha) = opt(&pull["head"]["sha"]) else {
        return Ok(None);
    };
    let runs = ctx
        .api
        .json(
            &format!("{base}/repos/{project}/commits/{sha}/check-runs"),
            &[("per_page", "100")],
        )
        .await?;
    let Some(runs) = runs["check_runs"]
        .as_array()
        .filter(|runs| !runs.is_empty())
    else {
        return Ok(None);
    };
    let mut jobs = Vec::new();
    let (mut failed, mut running, mut pending, mut canceled) = (false, false, false, false);
    for run in runs {
        let status = text(&run["status"]);
        let conclusion = opt(&run["conclusion"]);
        match (status.as_str(), conclusion.as_deref()) {
            (
                "completed",
                Some("failure" | "timed_out" | "action_required" | "startup_failure"),
            ) => failed = true,
            ("completed", Some("cancelled")) => canceled = true,
            ("in_progress", _) => running = true,
            ("completed", _) => {}
            _ => pending = true,
        }
        let duration = match (
            epoch(run["started_at"].as_str().unwrap_or("")),
            epoch(run["completed_at"].as_str().unwrap_or("")),
        ) {
            (Some(start), Some(end)) if end >= start => Some((end - start) as f64),
            _ => None,
        };
        jobs.push(Job {
            id: run["id"].to_string(),
            name: text(&run["name"]),
            stage: opt(&run["app"]["name"]).unwrap_or_default(),
            status: conclusion.unwrap_or(status),
            url: text(&run["html_url"]),
            duration_s: duration,
        });
    }
    let state = if failed {
        PipelineState::Failed
    } else if running {
        PipelineState::Running
    } else if pending {
        PipelineState::Pending
    } else if canceled {
        PipelineState::Canceled
    } else {
        PipelineState::Success
    };
    Ok(Some(Pipeline {
        id: sha,
        status: state,
        url: format!("{}/checks", text(&pull["html_url"])),
        jobs,
    }))
}
