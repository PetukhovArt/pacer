use serde::Serialize;
use serde_json::Value;

use super::http::{encode_segment, opt, text, ApiError, ApiResult};
use super::types::{Kind, Person};
use super::{Connectors, Ctx};

#[derive(Debug, Serialize)]
pub struct ConfluenceHit {
    connector: String,
    id: String,
    title: String,
    space: String,
    url: String,
    excerpt: String,
    updated: String,
}

#[derive(Debug, Serialize)]
pub struct Ancestor {
    id: String,
    title: String,
}

#[derive(Debug, Serialize)]
pub struct ConfluencePage {
    connector: String,
    id: String,
    title: String,
    space: String,
    url: String,
    version: u64,
    body_html: String,
    ancestors: Vec<Ancestor>,
}

fn origin(ctx: &Ctx) -> String {
    match reqwest::Url::parse(&ctx.base_url) {
        Ok(url) => {
            let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
            format!("{}://{}{port}", url.scheme(), url.host_str().unwrap_or(""))
        }
        Err(_) => ctx.base_url.clone(),
    }
}

fn is_cloud(ctx: &Ctx) -> bool {
    reqwest::Url::parse(&ctx.base_url)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.ends_with(".atlassian.net")))
        .unwrap_or(false)
}

fn root(ctx: &Ctx) -> String {
    if is_cloud(ctx) {
        format!("{}/wiki", origin(ctx))
    } else {
        ctx.base_url.clone()
    }
}

fn api(ctx: &Ctx) -> String {
    format!("{}/rest/api", root(ctx))
}

fn absolute(ctx: &Ctx, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_owned()
    } else {
        format!("{}{path}", root(ctx))
    }
}

fn strip_highlight(raw: &str) -> String {
    raw.replace("@@@hl@@@", "")
        .replace("@@@endhl@@@", "")
        .trim()
        .to_owned()
}

fn cql_quote(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

pub async fn whoami(ctx: &Ctx) -> ApiResult<Person> {
    let me = ctx
        .api
        .json(&format!("{}/user/current", api(ctx)), &[])
        .await?;
    let name = opt(&me["displayName"])
        .or_else(|| opt(&me["username"]))
        .ok_or_else(|| ApiError::failed("неожиданный ответ Confluence: нет данных пользователя"))?;
    let username = opt(&me["username"])
        .or_else(|| opt(&me["userKey"]))
        .or_else(|| opt(&me["accountId"]))
        .unwrap_or_else(|| name.clone());
    let avatar = opt(&me["profilePicture"]["path"]).map(|path| {
        if path.starts_with("http") {
            path
        } else if is_cloud(ctx) {
            format!("{}{path}", origin(ctx))
        } else {
            format!("{}{path}", ctx.base_url)
        }
    });
    Ok(Person {
        name,
        username,
        avatar_url: avatar,
    })
}

pub async fn confluence_search(
    state: &Connectors,
    connector: String,
    text: String,
    limit: Option<u32>,
) -> Result<Vec<ConfluenceHit>, String> {
    let ctx = state.ctx(&connector, &[Kind::Confluence]).await?;
    let limit = limit.unwrap_or(25).clamp(1, 100).to_string();
    let cql = format!(
        "type = page AND text ~ \"{}\" ORDER BY lastmodified DESC",
        cql_quote(&text)
    );
    let primary = ctx
        .api
        .json(
            &format!("{}/search", api(&ctx)),
            &[("cql", cql.as_str()), ("limit", limit.as_str())],
        )
        .await;
    let hits = match primary {
        Ok(found) => found["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| hit_from_search(&ctx, item))
            .collect(),
        Err(error) if error.message().contains("(404)") => {
            let found = ctx
                .api
                .json(
                    &format!("{}/content/search", api(&ctx)),
                    &[
                        ("cql", cql.as_str()),
                        ("limit", limit.as_str()),
                        ("expand", "space,version"),
                    ],
                )
                .await?;
            found["results"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| hit_from_content(&ctx, item))
                .collect()
        }
        Err(error) => return Err(error.into()),
    };
    Ok(hits)
}

fn hit_from_search(ctx: &Ctx, item: &Value) -> Option<ConfluenceHit> {
    let id = opt(&item["content"]["id"])?;
    Some(ConfluenceHit {
        connector: ctx.id.clone(),
        id,
        title: strip_highlight(&text(&item["title"])),
        space: opt(&item["resultGlobalContainer"]["title"])
            .or_else(|| opt(&item["content"]["space"]["name"]))
            .unwrap_or_default(),
        url: absolute(ctx, &text(&item["url"])),
        excerpt: strip_highlight(&text(&item["excerpt"])),
        updated: opt(&item["lastModified"])
            .or_else(|| opt(&item["content"]["version"]["when"]))
            .unwrap_or_default(),
    })
}

fn hit_from_content(ctx: &Ctx, item: &Value) -> Option<ConfluenceHit> {
    let id = opt(&item["id"])?;
    Some(ConfluenceHit {
        connector: ctx.id.clone(),
        id,
        title: text(&item["title"]),
        space: opt(&item["space"]["name"])
            .or_else(|| opt(&item["space"]["key"]))
            .unwrap_or_default(),
        url: absolute(ctx, &text(&item["_links"]["webui"])),
        excerpt: String::new(),
        updated: text(&item["version"]["when"]),
    })
}

pub async fn confluence_page(
    state: &Connectors,
    connector: String,
    id: String,
) -> Result<ConfluencePage, String> {
    let ctx = state.ctx(&connector, &[Kind::Confluence]).await?;
    let page = ctx
        .api
        .json(
            &format!("{}/content/{}", api(&ctx), encode_segment(&id)),
            &[("expand", "body.view,space,version,ancestors")],
        )
        .await?;
    let webui = text(&page["_links"]["webui"]);
    let url = match opt(&page["_links"]["base"]) {
        Some(base) => format!("{base}{webui}"),
        None => absolute(&ctx, &webui),
    };
    Ok(ConfluencePage {
        connector: ctx.id.clone(),
        id: opt(&page["id"]).unwrap_or(id),
        title: text(&page["title"]),
        space: opt(&page["space"]["name"])
            .or_else(|| opt(&page["space"]["key"]))
            .unwrap_or_default(),
        url,
        version: page["version"]["number"].as_u64().unwrap_or(0),
        body_html: text(&page["body"]["view"]["value"]),
        ancestors: page["ancestors"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|ancestor| Ancestor {
                id: text(&ancestor["id"]),
                title: text(&ancestor["title"]),
            })
            .collect(),
    })
}
