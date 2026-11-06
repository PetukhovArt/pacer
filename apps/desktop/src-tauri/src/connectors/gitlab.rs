use futures_util::stream::{self, StreamExt};
use serde_json::Value;

use super::http::{encode_segment, opt, text, ApiError, ApiResult};
use super::types::{
    count_patch, ChangeRequest, ChangeRequestSummary, CrState, DiffFile, Discussion, FileStatus,
    Fork, InboxItem, Job, Note, Person, Pipeline, PipelineState, Position, Reason, SourceRepo,
};
use super::Ctx;

const PAGE: usize = 100;
const MAX_PAGES: usize = 20;
const ENRICH_PARALLEL: usize = 8;

fn api(ctx: &Ctx) -> String {
    format!("{}/api/v4", ctx.base_url)
}

fn mr_url(ctx: &Ctx, project: &str, number: u64) -> String {
    format!(
        "{}/projects/{}/merge_requests/{number}",
        api(ctx),
        encode_segment(project)
    )
}

fn person(ctx: &Ctx, user: &Value) -> Person {
    let username = text(&user["username"]);
    if username.is_empty() {
        return Person::ghost();
    }
    let avatar = opt(&user["avatar_url"]).map(|url| {
        if url.starts_with('/') {
            format!("{}{url}", ctx.base_url)
        } else {
            url
        }
    });
    Person {
        name: opt(&user["name"]).unwrap_or_else(|| username.clone()),
        username,
        avatar_url: avatar,
    }
}

pub async fn whoami(ctx: &Ctx) -> ApiResult<Person> {
    let user = ctx.api.json(&format!("{}/user", api(ctx)), &[]).await?;
    Ok(person(ctx, &user))
}

fn pipeline_state(status: &str) -> PipelineState {
    match status {
        "success" => PipelineState::Success,
        "failed" => PipelineState::Failed,
        "running" => PipelineState::Running,
        "pending" | "created" | "waiting_for_resource" | "preparing" | "scheduled" | "manual" => {
            PipelineState::Pending
        }
        "canceled" | "canceling" => PipelineState::Canceled,
        _ => PipelineState::None,
    }
}

fn project_of(ctx: &Ctx, web_url: &str) -> Option<String> {
    let rest = web_url.strip_prefix(ctx.base_url.as_str())?;
    let rest = rest.trim_start_matches('/');
    let cut = rest
        .find("/-/merge_requests/")
        .or_else(|| rest.find("/merge_requests/"))?;
    Some(rest[..cut].to_owned())
}

fn summary(ctx: &Ctx, mr: &Value, project: &str) -> Option<ChangeRequestSummary> {
    Some(ChangeRequestSummary {
        connector: ctx.id.clone(),
        project: project.to_owned(),
        number: mr["iid"].as_u64()?,
        title: text(&mr["title"]),
        url: text(&mr["web_url"]),
        state: match mr["state"].as_str() {
            Some("merged") => CrState::Merged,
            Some("closed") | Some("locked") => CrState::Closed,
            _ => CrState::Open,
        },
        draft: mr["draft"]
            .as_bool()
            .or_else(|| mr["work_in_progress"].as_bool())
            .unwrap_or(false),
        author: person(ctx, &mr["author"]),
        source_branch: text(&mr["source_branch"]),
        target_branch: text(&mr["target_branch"]),
        updated_at: text(&mr["updated_at"]),
        pipeline: mr["head_pipeline"]["status"]
            .as_str()
            .map(pipeline_state)
            .unwrap_or(PipelineState::None),
        unresolved_threads: None,
    })
}

fn is_unresolved(discussion: &Value) -> bool {
    discussion["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|note| {
            note["resolvable"] == Value::Bool(true) && note["resolved"] != Value::Bool(true)
        })
}

async fn all_pages(ctx: &Ctx, url: &str, extra: &[(&str, &str)]) -> ApiResult<Vec<Value>> {
    let per_page = PAGE.to_string();
    let mut found = Vec::new();
    for page in 1..=MAX_PAGES {
        let page_text = page.to_string();
        let mut query: Vec<(&str, &str)> = vec![("per_page", &per_page), ("page", &page_text)];
        query.extend_from_slice(extra);
        let batch = ctx.api.json(url, &query).await?;
        let entries = batch.as_array().cloned().unwrap_or_default();
        let count = entries.len();
        found.extend(entries);
        if count < PAGE {
            break;
        }
    }
    Ok(found)
}

async fn unresolved(ctx: &Ctx, project: &str, number: u64) -> Option<u32> {
    let url = format!("{}/discussions", mr_url(ctx, project, number));
    let discussions = all_pages(ctx, &url, &[]).await.ok()?;
    Some(discussions.iter().filter(|d| is_unresolved(d)).count() as u32)
}

async fn enrich(ctx: &Ctx, mut base: ChangeRequestSummary) -> ChangeRequestSummary {
    let detail_url = mr_url(ctx, &base.project, base.number);
    let (detail, count) = tokio::join!(
        ctx.api.json(&detail_url, &[]),
        unresolved(ctx, &base.project, base.number)
    );
    if let Ok(detail) = detail {
        if let Some(status) = detail["head_pipeline"]["status"].as_str() {
            base.pipeline = pipeline_state(status);
        }
    }
    base.unresolved_threads = count;
    base
}

async fn list(ctx: &Ctx, query: &[(&str, &str)]) -> ApiResult<Vec<Value>> {
    let mut full: Vec<(&str, &str)> = vec![("state", "opened"), ("per_page", "50")];
    full.extend_from_slice(query);
    let url = format!("{}/merge_requests", api(ctx));
    let found = ctx.api.json(&url, &full).await?;
    Ok(found.as_array().cloned().unwrap_or_default())
}

pub async fn inbox(ctx: &Ctx) -> ApiResult<Vec<InboxItem>> {
    let me = whoami(ctx).await?.username;
    let created = [("scope", "created_by_me")];
    let reviewer = [("scope", "all"), ("reviewer_username", me.as_str())];
    let assignee = [("scope", "assigned_to_me")];
    let (authored, reviewing, assigned) = tokio::try_join!(
        list(ctx, &created),
        list(ctx, &reviewer),
        list(ctx, &assignee)
    )?;
    let mut items: Vec<InboxItem> = Vec::new();
    for (batch, reason) in [
        (authored, Reason::Author),
        (reviewing, Reason::Reviewer),
        (assigned, Reason::Assignee),
    ] {
        for mr in &batch {
            let Some(project) = project_of(ctx, &text(&mr["web_url"])) else {
                continue;
            };
            let Some(found) = summary(ctx, mr, &project) else {
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
    let items = stream::iter(items)
        .map(|item| async move {
            InboxItem {
                summary: enrich(ctx, item.summary).await,
                reasons: item.reasons,
            }
        })
        .buffer_unordered(ENRICH_PARALLEL)
        .collect::<Vec<_>>()
        .await;
    Ok(items)
}

pub async fn for_branch(
    ctx: &Ctx,
    project: &str,
    branch: &str,
) -> ApiResult<Option<ChangeRequestSummary>> {
    let url = format!(
        "{}/projects/{}/merge_requests",
        api(ctx),
        encode_segment(project)
    );
    let found = ctx
        .api
        .json(
            &url,
            &[
                ("source_branch", branch),
                ("state", "opened"),
                ("order_by", "updated_at"),
                ("per_page", "1"),
            ],
        )
        .await?;
    let Some(mr) = found.get(0).filter(|mr| mr.is_object()) else {
        return Ok(None);
    };
    match summary(ctx, mr, project) {
        Some(base) => Ok(Some(enrich(ctx, base).await)),
        None => Ok(None),
    }
}

fn absolutize(ctx: &Ctx, project: &str, body: &str) -> String {
    body.replace(
        "](/uploads/",
        &format!("]({}/{project}/uploads/", ctx.base_url),
    )
}

pub async fn change_request(ctx: &Ctx, project: &str, number: u64) -> ApiResult<ChangeRequest> {
    let mr = ctx.api.json(&mr_url(ctx, project, number), &[]).await?;
    let mut found = summary(ctx, &mr, project)
        .ok_or_else(|| ApiError::failed("неожиданный ответ GitLab: нет данных MR"))?;
    found.unresolved_threads = unresolved(ctx, project, number).await;
    let mergeable = match mr["detailed_merge_status"].as_str() {
        Some("mergeable") => Some(true),
        Some("checking") | Some("unchecked") | Some("preparing") | Some("approvals_syncing") => {
            None
        }
        Some(_) => Some(false),
        None => match mr["merge_status"].as_str() {
            Some("can_be_merged") => Some(true),
            Some("cannot_be_merged") => Some(false),
            _ => None,
        },
    };
    Ok(ChangeRequest {
        summary: found,
        description: absolutize(ctx, project, &text(&mr["description"])),
        reviewers: mr["reviewers"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|user| person(ctx, user))
            .collect(),
        labels: mr["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|label| label.as_str().map(str::to_owned))
            .collect(),
        created_at: text(&mr["created_at"]),
        mergeable,
        head_sha: opt(&mr["diff_refs"]["head_sha"])
            .or_else(|| opt(&mr["sha"]))
            .unwrap_or_default(),
    })
}

const DEVELOPER_ACCESS: u64 = 30;

fn access_level(project: &Value) -> u64 {
    let permissions = &project["permissions"];
    ["project_access", "group_access"]
        .iter()
        .filter_map(|scope| permissions[scope]["access_level"].as_u64())
        .max()
        .unwrap_or(0)
}

async fn project_by_id(ctx: &Ctx, id: u64) -> ApiResult<Value> {
    ctx.api
        .json(&format!("{}/projects/{id}", api(ctx)), &[])
        .await
}

pub async fn source(ctx: &Ctx, project: &str, number: u64) -> ApiResult<SourceRepo> {
    let mr = ctx.api.json(&mr_url(ctx, project, number), &[]).await?;
    let branch = text(&mr["source_branch"]);
    let target_id = mr["target_project_id"].as_u64();
    let Some(source_id) = mr["source_project_id"]
        .as_u64()
        .filter(|id| Some(*id) != target_id)
    else {
        return Ok(SourceRepo {
            branch,
            fork: None,
            gone: false,
            can_push: None,
        });
    };
    let source = match project_by_id(ctx, source_id).await {
        Ok(found) => found,
        Err(ApiError::Failed(message)) if message.contains("(404)") => {
            return Ok(SourceRepo {
                branch,
                fork: None,
                gone: true,
                can_push: None,
            })
        }
        Err(error) => return Err(error),
    };
    let collaboration = mr["allow_collaboration"].as_bool().unwrap_or(false);
    let target_access = match (collaboration, target_id) {
        (true, Some(id)) => project_by_id(ctx, id)
            .await
            .map(|target| access_level(&target))
            .unwrap_or(0),
        _ => 0,
    };
    let can_push = access_level(&source) >= DEVELOPER_ACCESS
        || (collaboration && target_access >= DEVELOPER_ACCESS);
    let owner = opt(&source["namespace"]["full_path"])
        .or_else(|| {
            opt(&source["path_with_namespace"])
                .and_then(|path| path.rsplit_once('/').map(|(owner, _)| owner.to_owned()))
        })
        .unwrap_or_else(|| format!("fork-{source_id}"));
    Ok(SourceRepo {
        branch,
        fork: Some(Fork {
            owner: owner.replace('/', "-"),
            http_url: text(&source["http_url_to_repo"]),
            ssh_url: text(&source["ssh_url_to_repo"]),
        }),
        gone: false,
        can_push: Some(can_push),
    })
}

pub async fn file(ctx: &Ctx, project: &str, git_ref: &str, path: &str) -> ApiResult<String> {
    let url = format!(
        "{}/projects/{}/repository/files/{}/raw",
        api(ctx),
        encode_segment(project),
        encode_segment(path)
    );
    ctx.api.get_text(&url, &[("ref", git_ref)], "*/*").await
}

fn diff_file(entry: &Value) -> DiffFile {
    let new_path = text(&entry["new_path"]);
    let old_path = opt(&entry["old_path"]).unwrap_or_else(|| new_path.clone());
    let status = if entry["new_file"] == Value::Bool(true) {
        FileStatus::Added
    } else if entry["deleted_file"] == Value::Bool(true) {
        FileStatus::Deleted
    } else if entry["renamed_file"] == Value::Bool(true) {
        FileStatus::Renamed
    } else {
        FileStatus::Modified
    };
    let patch = text(&entry["diff"]);
    let too_large = patch.is_empty()
        && (entry["too_large"] == Value::Bool(true) || entry["collapsed"] == Value::Bool(true));
    let (additions, deletions) = count_patch(&patch);
    DiffFile {
        old_path,
        new_path,
        status,
        binary: patch.is_empty() && !too_large && status != FileStatus::Renamed,
        patch,
        additions,
        deletions,
        too_large,
    }
}

pub async fn diff(ctx: &Ctx, project: &str, number: u64) -> ApiResult<Vec<DiffFile>> {
    let base = mr_url(ctx, project, number);
    match all_pages(ctx, &format!("{base}/diffs"), &[]).await {
        Ok(entries) => Ok(entries.iter().map(diff_file).collect()),
        Err(error)
            if !["(401)", "(403)"]
                .iter()
                .any(|code| error.message().contains(code)) =>
        {
            let changes = ctx
                .api
                .json(&format!("{base}/changes"), &[("access_raw_diffs", "true")])
                .await?;
            Ok(changes["changes"]
                .as_array()
                .into_iter()
                .flatten()
                .map(diff_file)
                .collect())
        }
        Err(error) => Err(error),
    }
}

pub async fn discussions(ctx: &Ctx, project: &str, number: u64) -> ApiResult<Vec<Discussion>> {
    let url = format!("{}/discussions", mr_url(ctx, project, number));
    let raw = all_pages(ctx, &url, &[]).await?;
    let mut found = Vec::new();
    for discussion in &raw {
        let notes_raw = discussion["notes"].as_array().cloned().unwrap_or_default();
        if notes_raw.is_empty() {
            continue;
        }
        let resolvable = notes_raw
            .iter()
            .any(|note| note["resolvable"] == Value::Bool(true));
        let resolved = resolvable
            && notes_raw
                .iter()
                .filter(|note| note["resolvable"] == Value::Bool(true))
                .all(|note| note["resolved"] == Value::Bool(true));
        let position = notes_raw
            .iter()
            .map(|note| &note["position"])
            .find(|position| position.is_object())
            .map(|position| Position {
                path: opt(&position["new_path"])
                    .or_else(|| opt(&position["old_path"]))
                    .unwrap_or_default(),
                new_line: position["new_line"].as_u64().map(|n| n as u32),
                old_line: position["old_line"].as_u64().map(|n| n as u32),
            });
        found.push(Discussion {
            id: text(&discussion["id"]),
            resolvable,
            resolved,
            position,
            notes: notes_raw
                .iter()
                .map(|note| Note {
                    id: note["id"].to_string(),
                    author: person(ctx, &note["author"]),
                    body: absolutize(ctx, project, &text(&note["body"])),
                    created_at: text(&note["created_at"]),
                    system: note["system"] == Value::Bool(true),
                })
                .collect(),
        });
    }
    Ok(found)
}

pub async fn pipeline(ctx: &Ctx, project: &str, number: u64) -> ApiResult<Option<Pipeline>> {
    let base = mr_url(ctx, project, number);
    let mr = ctx.api.json(&base, &[]).await?;
    let mut head = mr["head_pipeline"].clone();
    if !head.is_object() {
        let list = ctx
            .api
            .json(&format!("{base}/pipelines"), &[("per_page", "1")])
            .await?;
        head = list.get(0).cloned().unwrap_or(Value::Null);
    }
    let Some(id) = head["id"].as_u64() else {
        return Ok(None);
    };
    let jobs_url = format!(
        "{}/projects/{}/pipelines/{id}/jobs",
        api(ctx),
        encode_segment(project)
    );
    let jobs = all_pages(ctx, &jobs_url, &[]).await?;
    Ok(Some(Pipeline {
        id: id.to_string(),
        status: pipeline_state(head["status"].as_str().unwrap_or("")),
        url: text(&head["web_url"]),
        jobs: jobs
            .iter()
            .map(|job| Job {
                id: job["id"].to_string(),
                name: text(&job["name"]),
                stage: text(&job["stage"]),
                status: text(&job["status"]),
                url: text(&job["web_url"]),
                duration_s: job["duration"].as_f64(),
            })
            .collect(),
    }))
}
