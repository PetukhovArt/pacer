use serde::Serialize;
use serde_json::Value;
use tauri::State;

use super::http::{encode_segment, opt, text, ApiError, ApiResult};
use super::types::{Kind, Person};
use super::{Connectors, Ctx};

const FIELDS: &str = "summary,status,priority,issuetype,assignee,updated";

#[derive(Debug, Serialize)]
pub struct JiraIssueSummary {
    connector: String,
    key: String,
    summary: String,
    status: String,
    status_category: &'static str,
    priority: Option<String>,
    issue_type: String,
    assignee: Option<Person>,
    updated: String,
    url: String,
}

#[derive(Debug, Serialize)]
pub struct JiraComment {
    id: String,
    author: Person,
    body_html: String,
    created: String,
}

#[derive(Debug, Serialize)]
pub struct JiraLink {
    relation: String,
    key: String,
    summary: String,
    status: String,
}

#[derive(Debug, Serialize)]
pub struct JiraIssue {
    #[serde(flatten)]
    summary: JiraIssueSummary,
    description_html: String,
    reporter: Option<Person>,
    labels: Vec<String>,
    fix_versions: Vec<String>,
    components: Vec<String>,
    comments: Vec<JiraComment>,
    links: Vec<JiraLink>,
    subtasks: Vec<JiraIssueSummary>,
}

#[derive(Debug, Serialize)]
pub struct JiraSearch {
    total: u64,
    issues: Vec<JiraIssueSummary>,
}

fn is_cloud(ctx: &Ctx) -> bool {
    reqwest::Url::parse(&ctx.base_url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.ends_with(".atlassian.net")))
        .unwrap_or(false)
}

fn api(ctx: &Ctx) -> String {
    format!("{}/rest/api/2", ctx.base_url)
}

fn person(user: &Value) -> Option<Person> {
    if !user.is_object() {
        return None;
    }
    let name = opt(&user["displayName"]).or_else(|| opt(&user["name"]))?;
    let username = opt(&user["name"])
        .or_else(|| opt(&user["key"]))
        .or_else(|| opt(&user["accountId"]))
        .or_else(|| opt(&user["emailAddress"]))
        .unwrap_or_else(|| name.clone());
    let avatar = &user["avatarUrls"];
    Some(Person {
        name,
        username,
        avatar_url: opt(&avatar["48x48"]).or_else(|| opt(&avatar["32x32"])),
    })
}

pub async fn whoami(ctx: &Ctx) -> ApiResult<Person> {
    let me = ctx.api.json(&format!("{}/myself", api(ctx)), &[]).await?;
    person(&me).ok_or_else(|| ApiError::failed("неожиданный ответ Jira: нет данных пользователя"))
}

fn category(status: &Value) -> &'static str {
    match status["statusCategory"]["key"].as_str() {
        Some("done") => "done",
        Some("indeterminate") => "in_progress",
        _ => "todo",
    }
}

fn summary(ctx: &Ctx, issue: &Value) -> Option<JiraIssueSummary> {
    let key = opt(&issue["key"])?;
    let fields = &issue["fields"];
    Some(JiraIssueSummary {
        connector: ctx.id.clone(),
        url: format!("{}/browse/{key}", ctx.base_url),
        key,
        summary: text(&fields["summary"]),
        status: text(&fields["status"]["name"]),
        status_category: category(&fields["status"]),
        priority: opt(&fields["priority"]["name"]),
        issue_type: text(&fields["issuetype"]["name"]),
        assignee: person(&fields["assignee"]),
        updated: text(&fields["updated"]),
    })
}

fn escape_html(raw: &str) -> String {
    let escaped = raw
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("<p>{}</p>", escaped.replace('\n', "<br>"))
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| opt(&item["name"]))
        .collect()
}

#[tauri::command]
pub async fn jira_search(
    state: State<'_, Connectors>,
    connector: String,
    jql: String,
    start: Option<u32>,
    limit: Option<u32>,
) -> Result<JiraSearch, String> {
    let ctx = state.ctx(&connector, &[Kind::Jira]).await?;
    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let (start_text, limit_text) = (start.to_string(), limit.to_string());
    let found = if is_cloud(&ctx) {
        ctx.api
            .json(
                &format!("{}/search/jql", api(&ctx)),
                &[
                    ("jql", jql.as_str()),
                    ("maxResults", limit_text.as_str()),
                    ("fields", FIELDS),
                ],
            )
            .await?
    } else {
        ctx.api
            .json(
                &format!("{}/search", api(&ctx)),
                &[
                    ("jql", jql.as_str()),
                    ("startAt", start_text.as_str()),
                    ("maxResults", limit_text.as_str()),
                    ("fields", FIELDS),
                ],
            )
            .await?
    };
    let issues: Vec<JiraIssueSummary> = found["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|issue| summary(&ctx, issue))
        .collect();
    let total = found["total"].as_u64().unwrap_or_else(|| {
        let more = u64::from(found["isLast"] == Value::Bool(false));
        u64::from(start) + issues.len() as u64 + more
    });
    Ok(JiraSearch { total, issues })
}

#[tauri::command]
pub async fn jira_issue(
    state: State<'_, Connectors>,
    connector: String,
    key: String,
) -> Result<JiraIssue, String> {
    let ctx = state.ctx(&connector, &[Kind::Jira]).await?;
    let issue = ctx
        .api
        .json(
            &format!("{}/issue/{}", api(&ctx), encode_segment(&key)),
            &[("expand", "renderedFields")],
        )
        .await?;
    let head = summary(&ctx, &issue)
        .ok_or_else(|| String::from("неожиданный ответ Jira: нет данных задачи"))?;
    let fields = &issue["fields"];
    let rendered = &issue["renderedFields"];
    let rendered_comments = rendered["comment"]["comments"].as_array();
    let comments = fields["comment"]["comments"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, comment)| {
            let html = rendered_comments
                .and_then(|list| list.get(index))
                .and_then(|entry| opt(&entry["body"]))
                .unwrap_or_else(|| escape_html(&text(&comment["body"])));
            JiraComment {
                id: text(&comment["id"]),
                author: person(&comment["author"]).unwrap_or_else(Person::ghost),
                body_html: html,
                created: text(&comment["created"]),
            }
        })
        .collect();
    let links = fields["issuelinks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|link| {
            let (other, relation) = if link["outwardIssue"].is_object() {
                (&link["outwardIssue"], &link["type"]["outward"])
            } else {
                (&link["inwardIssue"], &link["type"]["inward"])
            };
            Some(JiraLink {
                relation: opt(relation).or_else(|| opt(&link["type"]["name"]))?,
                key: opt(&other["key"])?,
                summary: text(&other["fields"]["summary"]),
                status: text(&other["fields"]["status"]["name"]),
            })
        })
        .collect();
    let subtasks = fields["subtasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|task| summary(&ctx, task))
        .collect();
    Ok(JiraIssue {
        summary: head,
        description_html: opt(&rendered["description"]).unwrap_or_else(|| {
            match opt(&fields["description"]) {
                Some(plain) => escape_html(&plain),
                None => String::new(),
            }
        }),
        reporter: person(&fields["reporter"]),
        labels: fields["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|label| label.as_str().map(str::to_owned))
            .collect(),
        fix_versions: names(&fields["fixVersions"]),
        components: names(&fields["components"]),
        comments,
        links,
        subtasks,
    })
}
