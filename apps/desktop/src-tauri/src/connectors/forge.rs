use futures_util::future::join_all;
use reqwest::Url;
use tauri::State;

use super::http::ApiResult;
use super::types::{
    ChangeRequest, ChangeRequestSummary, DiffFile, Discussion, Inbox, InboxError, InboxItem, Kind,
    Pipeline,
};
use super::{cli, github, gitlab, Connectors, Ctx};

const FORGES: [Kind; 2] = [Kind::Github, Kind::Gitlab];

async fn forge_ctx(state: &Connectors, connector: &str) -> Result<Ctx, String> {
    state.ctx(connector, &FORGES).await
}

async fn inbox_of(state: &Connectors, id: &str) -> Result<Vec<InboxItem>, String> {
    let ctx = forge_ctx(state, id).await?;
    let items: ApiResult<Vec<InboxItem>> = match ctx.kind {
        Kind::Github => github::inbox(&ctx).await,
        _ => gitlab::inbox(&ctx).await,
    };
    items.map_err(String::from)
}

#[tauri::command]
pub async fn forge_inbox(state: State<'_, Connectors>) -> Result<Inbox, String> {
    let records = state.forge_records();
    let results = join_all(records.iter().map(|record| inbox_of(&state, &record.id))).await;
    let mut items = Vec::new();
    let mut errors = Vec::new();
    for (record, result) in records.iter().zip(results) {
        match result {
            Ok(found) => items.extend(found),
            Err(message) => errors.push(InboxError {
                connector: record.id.clone(),
                message,
            }),
        }
    }
    items.sort_by(|a, b| b.summary.updated_at.cmp(&a.summary.updated_at));
    Ok(Inbox { items, errors })
}

fn parse_remote(remote: &str) -> Option<(String, String)> {
    let remote = remote.trim();
    let (host, path) = if remote.contains("://") {
        let url = Url::parse(remote).ok()?;
        (url.host_str()?.to_owned(), url.path().to_owned())
    } else {
        let (left, path) = remote.split_once(':')?;
        let host = left.rsplit_once('@').map_or(left, |(_, host)| host);
        if host.len() < 2 {
            return None;
        }
        (host.to_owned(), path.to_owned())
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path).trim_matches('/');
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some((host.to_ascii_lowercase(), path.to_owned()))
}

fn base_parts(base_url: &str) -> Option<(String, String)> {
    let url = Url::parse(base_url).ok()?;
    Some((
        url.host_str()?.to_ascii_lowercase(),
        url.path().trim_matches('/').to_owned(),
    ))
}

#[tauri::command]
pub async fn forge_for_worktree(
    state: State<'_, Connectors>,
    path: String,
    branch: String,
) -> Result<Option<ChangeRequestSummary>, String> {
    let output = match cli::capture("git", &["-C", &path, "remote", "get-url", "origin"]).await {
        Ok(output) if output.status.success() => output,
        _ => return Ok(None),
    };
    let remote = String::from_utf8_lossy(&output.stdout).into_owned();
    let Some((host, remote_path)) = parse_remote(&remote) else {
        return Ok(None);
    };
    let found = state.forge_records().into_iter().find_map(|record| {
        let (base_host, prefix) = base_parts(&record.base_url)?;
        (base_host == host).then_some((record, prefix))
    });
    let Some((record, prefix)) = found else {
        return Ok(None);
    };
    let project = if prefix.is_empty() {
        remote_path.as_str()
    } else {
        remote_path
            .strip_prefix(&format!("{prefix}/"))
            .unwrap_or(&remote_path)
    };
    let ctx = forge_ctx(&state, &record.id).await?;
    let summary = match ctx.kind {
        Kind::Github => github::for_branch(&ctx, project, &branch).await,
        _ => gitlab::for_branch(&ctx, project, &branch).await,
    };
    summary.map_err(String::from)
}

#[tauri::command]
pub async fn forge_change_request(
    state: State<'_, Connectors>,
    connector: String,
    project: String,
    number: u64,
) -> Result<ChangeRequest, String> {
    let ctx = forge_ctx(&state, &connector).await?;
    match ctx.kind {
        Kind::Github => github::change_request(&ctx, &project, number).await,
        _ => gitlab::change_request(&ctx, &project, number).await,
    }
    .map_err(String::from)
}

#[tauri::command]
pub async fn forge_diff(
    state: State<'_, Connectors>,
    connector: String,
    project: String,
    number: u64,
) -> Result<Vec<DiffFile>, String> {
    let ctx = forge_ctx(&state, &connector).await?;
    match ctx.kind {
        Kind::Github => github::diff(&ctx, &project, number).await,
        _ => gitlab::diff(&ctx, &project, number).await,
    }
    .map_err(String::from)
}

#[tauri::command]
pub async fn forge_discussions(
    state: State<'_, Connectors>,
    connector: String,
    project: String,
    number: u64,
) -> Result<Vec<Discussion>, String> {
    let ctx = forge_ctx(&state, &connector).await?;
    match ctx.kind {
        Kind::Github => github::discussions(&ctx, &project, number).await,
        _ => gitlab::discussions(&ctx, &project, number).await,
    }
    .map_err(String::from)
}

#[tauri::command]
pub async fn forge_pipeline(
    state: State<'_, Connectors>,
    connector: String,
    project: String,
    number: u64,
) -> Result<Option<Pipeline>, String> {
    let ctx = forge_ctx(&state, &connector).await?;
    match ctx.kind {
        Kind::Github => github::pipeline(&ctx, &project, number).await,
        _ => gitlab::pipeline(&ctx, &project, number).await,
    }
    .map_err(String::from)
}
