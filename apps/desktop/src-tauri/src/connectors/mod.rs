pub mod ai;
mod cli;
pub mod confluence;
pub mod forge;
mod github;
mod gitlab;
mod http;
pub mod jira;
pub mod monitor;
pub mod secrets;
pub mod types;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures_util::future::join_all;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use http::{Api, ApiError, ApiResult};
use types::{AuthMode, Kind, Person, Status};

const ASSET_LIMIT: u64 = 25 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub kind: Kind,
    pub label: String,
    pub base_url: String,
    pub auth: AuthMode,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
struct Health {
    status: Status,
    error: Option<String>,
    account: Option<Person>,
}

impl Health {
    fn unchecked() -> Self {
        Health {
            status: Status::Unchecked,
            error: None,
            account: None,
        }
    }

    fn from_result(result: &ApiResult<Person>) -> Self {
        match result {
            Ok(person) => Health {
                status: Status::Ok,
                error: None,
                account: Some(person.clone()),
            },
            Err(error @ ApiError::Unreachable(_)) => Health {
                status: Status::Unreachable,
                error: Some(error.message().to_owned()),
                account: None,
            },
            Err(error) => Health {
                status: Status::Error,
                error: Some(error.message().to_owned()),
                account: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorView {
    id: String,
    kind: Kind,
    label: String,
    base_url: String,
    auth: AuthMode,
    account: Option<Person>,
    status: Status,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Detected {
    kind: Kind,
    base_url: String,
    account: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Asset {
    mime: String,
    base64: String,
}

pub struct Ctx {
    pub id: String,
    pub kind: Kind,
    pub base_url: String,
    pub api: Api,
}

#[derive(Default)]
struct Inner {
    records: Vec<Record>,
    health: HashMap<String, Health>,
}

#[derive(Default)]
pub struct Connectors {
    inner: Mutex<Inner>,
    tokens: cli::TokenCache,
    ai: ai::AiState,
    hub: Arc<monitor::Hub>,
}

fn registry_path() -> Result<PathBuf, String> {
    Ok(crate::settings::berth_dir()?.join("connectors.json"))
}

fn persist(records: &[Record]) -> Result<(), String> {
    let path = registry_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("не удалось создать {}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(records).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text).map_err(|e| format!("не удалось записать реестр коннекторов: {e}"))?;
    fs::rename(&temp, &path).map_err(|e| format!("не удалось записать реестр коннекторов: {e}"))
}

fn view(record: &Record, health: Option<&Health>) -> ConnectorView {
    let unchecked = Health::unchecked();
    let health = health.unwrap_or(&unchecked);
    ConnectorView {
        id: record.id.clone(),
        kind: record.kind,
        label: record.label.clone(),
        base_url: record.base_url.clone(),
        auth: record.auth,
        account: health.account.clone(),
        status: health.status,
        error: health.error.clone(),
    }
}

fn normalize_base_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("не указан адрес сервера".into());
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| format!("некорректный адрес: {trimmed}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(format!("некорректный адрес: {trimmed}"));
    }
    let host = url.host_str().unwrap_or_default();
    let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
    let path = url.path().trim_end_matches('/');
    Ok(format!("{}://{host}{port}{path}", url.scheme()))
}

fn slug(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn headers_for(kind: Kind, email: Option<&str>, token: &str) -> Vec<(&'static str, String)> {
    match kind {
        Kind::Github => vec![
            ("Authorization", format!("Bearer {token}")),
            ("X-GitHub-Api-Version", "2022-11-28".to_owned()),
        ],
        Kind::Gitlab => vec![("PRIVATE-TOKEN", token.to_owned())],
        Kind::Jira | Kind::Confluence => match email {
            Some(email) => vec![(
                "Authorization",
                format!("Basic {}", STANDARD.encode(format!("{email}:{token}"))),
            )],
            None => vec![("Authorization", format!("Bearer {token}"))],
        },
    }
}

async fn whoami(ctx: &Ctx) -> ApiResult<Person> {
    match ctx.kind {
        Kind::Github => github::whoami(ctx).await,
        Kind::Gitlab => gitlab::whoami(ctx).await,
        Kind::Jira => jira::whoami(ctx).await,
        Kind::Confluence => confluence::whoami(ctx).await,
    }
}

impl Connectors {
    pub fn load() -> Self {
        let records = registry_path()
            .ok()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<Vec<Record>>(&text).ok())
            .unwrap_or_default();
        Connectors {
            inner: Mutex::new(Inner {
                records,
                health: HashMap::new(),
            }),
            tokens: cli::TokenCache::default(),
            ai: ai::AiState::default(),
            hub: Arc::default(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn records(&self) -> Vec<Record> {
        self.lock().records.clone()
    }

    fn record(&self, id: &str) -> Result<Record, String> {
        self.lock()
            .records
            .iter()
            .find(|record| record.id == id)
            .cloned()
            .ok_or_else(|| format!("коннектор {id} не найден"))
    }

    fn views(&self) -> Vec<ConnectorView> {
        let inner = self.lock();
        inner
            .records
            .iter()
            .map(|record| view(record, inner.health.get(&record.id)))
            .collect()
    }

    fn set_health(&self, id: &str, health: Health) {
        let mut inner = self.lock();
        if inner.records.iter().any(|record| record.id == id) {
            inner.health.insert(id.to_owned(), health);
        }
    }

    async fn build_ctx(&self, record: &Record, token: Option<&str>) -> ApiResult<Ctx> {
        let token = match token {
            Some(token) => token.to_owned(),
            None => match record.auth {
                AuthMode::Cli => self.tokens.token(record.kind, &record.base_url).await?,
                AuthMode::Token => secrets::load(&record.id)?,
            },
        };
        let hub = self.hub.clone();
        let id = record.id.clone();
        let api = Api::new(headers_for(record.kind, record.email.as_deref(), &token))
            .on_unreachable(Arc::new(move |message: &str| {
                hub.report_unreachable(&id, message)
            }));
        Ok(Ctx {
            id: record.id.clone(),
            kind: record.kind,
            base_url: record.base_url.clone(),
            api,
        })
    }

    pub async fn ctx(&self, id: &str, kinds: &[Kind]) -> Result<Ctx, String> {
        let record = self.record(id)?;
        if !kinds.contains(&record.kind) {
            return Err(format!(
                "коннектор «{}» — это {}, а не {}",
                record.label,
                record.kind.title(),
                kinds
                    .iter()
                    .map(|kind| kind.title())
                    .collect::<Vec<_>>()
                    .join(" или ")
            ));
        }
        self.build_ctx(&record, None).await.map_err(String::from)
    }

    pub fn forge_records(&self) -> Vec<Record> {
        self.records()
            .into_iter()
            .filter(|record| record.kind.is_forge())
            .collect()
    }

    async fn check_record(&self, record: &Record) -> ApiResult<Person> {
        let ctx = self.build_ctx(record, None).await?;
        whoami(&ctx).await
    }

    fn unique_id(&self, kind: Kind, base_url: &str) -> String {
        let host = cli::host_port(base_url).unwrap_or_else(|| base_url.to_owned());
        let stem = format!("{}-{}", slug(kind.title()), slug(&host));
        let inner = self.lock();
        let mut id = stem.clone();
        let mut n = 2;
        while inner.records.iter().any(|record| record.id == id) {
            id = format!("{stem}-{n}");
            n += 1;
        }
        id
    }
}

pub async fn connectors_list(state: &Connectors) -> Result<Vec<ConnectorView>, String> {
    Ok(state.views())
}

pub async fn connectors_check(state: &Connectors) -> Result<Vec<ConnectorView>, String> {
    let records = state.records();
    let results = join_all(records.iter().map(|record| state.check_record(record))).await;
    for (record, result) in records.iter().zip(results.iter()) {
        state.set_health(&record.id, Health::from_result(result));
    }
    state.publish();
    Ok(state.views())
}

pub async fn connectors_detect(state: &Connectors) -> Result<Vec<Detected>, String> {
    let existing = state.records();
    let known = |kind: Kind, base_url: &str| {
        let host = cli::host_port(base_url);
        existing
            .iter()
            .any(|record| record.kind == kind && cli::host_port(&record.base_url) == host)
    };
    Ok(cli::detect()
        .await
        .into_iter()
        .filter(|login| !known(login.kind, &login.base_url))
        .map(|login| Detected {
            kind: login.kind,
            base_url: login.base_url,
            account: login.account,
        })
        .collect())
}

pub async fn connector_add(
    state: &Connectors,
    kind: Kind,
    base_url: String,
    label: Option<String>,
    auth: AuthMode,
    token: Option<String>,
    email: Option<String>,
) -> Result<ConnectorView, String> {
    let base_url = normalize_base_url(&base_url)?;
    let token = token.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
    let email = email.map(|e| e.trim().to_owned()).filter(|e| !e.is_empty());
    match auth {
        AuthMode::Cli if !kind.is_forge() => {
            return Err(format!(
                "{}: вход через CLI недоступен, нужен токен",
                kind.title()
            ))
        }
        AuthMode::Token if token.is_none() => return Err("не указан токен".into()),
        _ => {}
    }
    let email = if matches!(kind, Kind::Jira | Kind::Confluence) {
        email
    } else {
        None
    };
    let host = cli::host_port(&base_url).unwrap_or_default();
    if auth == AuthMode::Cli
        && state.records().iter().any(|r| {
            r.kind == kind
                && r.auth == AuthMode::Cli
                && cli::host_port(&r.base_url).as_deref() == Some(host.as_str())
        })
    {
        return Err(format!("{host} уже добавлен"));
    }
    let record = Record {
        id: state.unique_id(kind, &base_url),
        kind,
        label: label
            .map(|l| l.trim().to_owned())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| format!("{} {host}", kind.title())),
        base_url,
        auth,
        email,
    };
    let token = if auth == AuthMode::Token { token } else { None };
    let ctx = state
        .build_ctx(&record, token.as_deref())
        .await
        .map_err(String::from)?;
    let person = whoami(&ctx).await.map_err(String::from)?;
    if let Some(token) = &token {
        secrets::save(&record.id, token)?;
    }
    let mut records = state.records();
    records.push(record.clone());
    if let Err(error) = persist(&records) {
        secrets::delete(&record.id);
        return Err(error);
    }
    {
        let mut inner = state.lock();
        inner.records.push(record.clone());
        inner
            .health
            .insert(record.id.clone(), Health::from_result(&Ok(person)));
    }
    state.publish();
    let inner = state.lock();
    Ok(view(&record, inner.health.get(&record.id)))
}

pub async fn connector_update(
    state: &Connectors,
    id: String,
    label: Option<String>,
    token: Option<String>,
    email: Option<String>,
) -> Result<ConnectorView, String> {
    let mut record = state.record(&id)?;
    let token = token.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
    if let Some(label) = label.map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()) {
        record.label = label;
    }
    let mut credentials_changed = token.is_some();
    if let Some(email) = email {
        if matches!(record.kind, Kind::Jira | Kind::Confluence) {
            let email = Some(email.trim().to_owned()).filter(|e| !e.is_empty());
            credentials_changed |= email != record.email;
            record.email = email;
        }
    }
    let mut health = None;
    if credentials_changed {
        let ctx = state
            .build_ctx(&record, token.as_deref())
            .await
            .map_err(String::from)?;
        let person = whoami(&ctx).await.map_err(String::from)?;
        health = Some(Health::from_result(&Ok(person)));
    }
    if let Some(token) = &token {
        secrets::save(&record.id, token)?;
        if record.auth == AuthMode::Cli {
            record.auth = AuthMode::Token;
        }
    }
    let mut records = state.records();
    if let Some(slot) = records.iter_mut().find(|r| r.id == id) {
        *slot = record.clone();
    }
    persist(&records)?;
    {
        let mut inner = state.lock();
        if let Some(slot) = inner.records.iter_mut().find(|r| r.id == id) {
            *slot = record.clone();
        }
        if let Some(health) = health {
            inner.health.insert(id.clone(), health);
        }
    }
    state.publish();
    let inner = state.lock();
    Ok(view(&record, inner.health.get(&id)))
}

pub async fn connector_remove(state: &Connectors, id: String) -> Result<(), String> {
    let mut records = state.records();
    let before = records.len();
    records.retain(|record| record.id != id);
    if records.len() == before {
        return Err(format!("коннектор {id} не найден"));
    }
    persist(&records)?;
    {
        let mut inner = state.lock();
        inner.records.retain(|record| record.id != id);
        inner.health.remove(&id);
    }
    secrets::delete(&id);
    state.publish();
    Ok(())
}

fn api_host(kind: Kind, base_url: &str) -> Option<String> {
    let host = Url::parse(base_url).ok()?.host_str()?.to_owned();
    Some(if kind == Kind::Github && host == "github.com" {
        "api.github.com".to_owned()
    } else {
        host
    })
}

fn asset_allowed(kind: Kind, base_host: &str, host: &str) -> bool {
    if host == base_host {
        return true;
    }
    kind == Kind::Github
        && (host == "api.github.com"
            || host == "github.com"
            || host.ends_with(".githubusercontent.com")
            || host == "githubusercontent.com")
}

pub async fn connector_asset(state: &Connectors, id: String, url: String) -> Result<Asset, String> {
    let ctx = state
        .ctx(
            &id,
            &[Kind::Github, Kind::Gitlab, Kind::Jira, Kind::Confluence],
        )
        .await?;
    let base =
        Url::parse(&ctx.base_url).map_err(|_| format!("некорректный адрес: {}", ctx.base_url))?;
    let target = base
        .join(url.trim())
        .map_err(|_| format!("некорректный адрес вложения: {url}"))?;
    let base_host = base.host_str().unwrap_or_default().to_owned();
    let host = target.host_str().unwrap_or_default().to_owned();
    if !matches!(target.scheme(), "http" | "https") || !asset_allowed(ctx.kind, &base_host, &host) {
        return Err(format!("вложение не с сервера коннектора ({base_host})"));
    }
    let own =
        host == base_host || Some(host.as_str()) == api_host(ctx.kind, &ctx.base_url).as_deref();
    let response = if own {
        ctx.api.get_with_auth(target).await
    } else {
        ctx.api.get_unauth(target).await
    }
    .map_err(String::from)?;
    if response
        .content_length()
        .is_some_and(|len| len > ASSET_LIMIT)
    {
        return Err("вложение слишком большое".into());
    }
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let bytes = response
        .bytes()
        .await
        .map_err(|_| format!("обрыв соединения с {host}"))?;
    if bytes.len() as u64 > ASSET_LIMIT {
        return Err("вложение слишком большое".into());
    }
    Ok(Asset {
        mime,
        base64: STANDARD.encode(&bytes),
    })
}
