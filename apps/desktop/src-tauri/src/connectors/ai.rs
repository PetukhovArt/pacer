use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;

use super::cli::capture;
use super::http::{Api, ApiError, ApiResult};
use super::types::Status;
use super::{secrets, Connectors};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiKind {
    Claude,
    Codex,
    Gemini,
    Openrouter,
    AnthropicApi,
    OpenaiApi,
    Grok,
}

const ALL: [AiKind; 7] = [
    AiKind::Claude,
    AiKind::Codex,
    AiKind::Gemini,
    AiKind::AnthropicApi,
    AiKind::OpenaiApi,
    AiKind::Openrouter,
    AiKind::Grok,
];

struct CliSpec {
    binary: &'static str,
    login: &'static [&'static str],
    logout: Option<&'static [&'static str]>,
}

struct KeySpec {
    env: &'static str,
    url: &'static str,
}

impl AiKind {
    fn label(self) -> &'static str {
        match self {
            AiKind::Claude => "Claude Code",
            AiKind::Codex => "Codex",
            AiKind::Gemini => "Gemini CLI",
            AiKind::Openrouter => "OpenRouter",
            AiKind::AnthropicApi => "Anthropic API",
            AiKind::OpenaiApi => "OpenAI API",
            AiKind::Grok => "Grok (xAI)",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            AiKind::Claude => "claude",
            AiKind::Codex => "codex",
            AiKind::Gemini => "gemini",
            AiKind::Openrouter => "openrouter",
            AiKind::AnthropicApi => "anthropic_api",
            AiKind::OpenaiApi => "openai_api",
            AiKind::Grok => "grok",
        }
    }

    fn cli(self) -> Option<CliSpec> {
        match self {
            AiKind::Claude => Some(CliSpec {
                binary: "claude",
                login: &["claude", "auth", "login"],
                logout: Some(&["claude", "auth", "logout"]),
            }),
            AiKind::Codex => Some(CliSpec {
                binary: "codex",
                login: &["codex", "login"],
                logout: Some(&["codex", "logout"]),
            }),
            AiKind::Gemini => Some(CliSpec {
                binary: "gemini",
                login: &["gemini"],
                logout: None,
            }),
            _ => None,
        }
    }

    fn key(self) -> Option<KeySpec> {
        match self {
            AiKind::Openrouter => Some(KeySpec {
                env: "OPENROUTER_API_KEY",
                url: "https://openrouter.ai/api/v1/key",
            }),
            AiKind::AnthropicApi => Some(KeySpec {
                env: "ANTHROPIC_API_KEY",
                url: "https://api.anthropic.com/v1/models",
            }),
            AiKind::OpenaiApi => Some(KeySpec {
                env: "OPENAI_API_KEY",
                url: "https://api.openai.com/v1/models",
            }),
            AiKind::Grok => Some(KeySpec {
                env: "XAI_API_KEY",
                url: "https://api.x.ai/v1/models",
            }),
            _ => None,
        }
    }

    fn secret_id(self) -> String {
        format!("ai-{}", self.slug())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiMethod {
    Cli,
    Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiStatus {
    SignedIn,
    SignedOut,
    NotInstalled,
    Rejected,
    Unreachable,
    Error,
    Unchecked,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiView {
    kind: AiKind,
    label: &'static str,
    method: AiMethod,
    status: AiStatus,
    account: Option<String>,
    plan: Option<String>,
    detail: Option<String>,
    version: Option<String>,
    expires_at: Option<String>,
    command: Option<String>,
}

impl AiView {
    fn base(kind: AiKind, status: AiStatus) -> Self {
        AiView {
            kind,
            label: kind.label(),
            method: if kind.cli().is_some() {
                AiMethod::Cli
            } else {
                AiMethod::Key
            },
            status,
            account: None,
            plan: None,
            detail: None,
            version: None,
            expires_at: None,
            command: kind.cli().map(|spec| spec.login.join(" ")),
        }
    }
}

#[derive(Default)]
pub struct AiState(Mutex<HashMap<AiKind, AiView>>);

impl AiState {
    fn get(&self, kind: AiKind) -> Option<AiView> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&kind)
            .cloned()
    }

    fn put(&self, view: &AiView) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(view.kind, view.clone());
    }

    fn drop_kind(&self, kind: AiKind) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&kind);
    }

    fn snapshot(&self, kind: AiKind) -> AiView {
        if let Some(spec) = kind.cli() {
            return self.get(kind).unwrap_or_else(|| {
                let status = if find_program(spec.binary).is_some() {
                    AiStatus::Unchecked
                } else {
                    AiStatus::NotInstalled
                };
                AiView::base(kind, status)
            });
        }
        match stored_key(kind) {
            None => AiView::base(kind, AiStatus::SignedOut),
            Some(_) => self
                .get(kind)
                .filter(|view| view.status != AiStatus::SignedOut)
                .unwrap_or_else(|| AiView::base(kind, AiStatus::Unchecked)),
        }
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    if let Some(home) = home() {
        for relative in [".local/bin", ".bun/bin", ".npm-global/bin", ".volta/bin"] {
            dirs.push(home.join(relative));
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs
}

fn find_program(name: &str) -> Option<PathBuf> {
    let extensions: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    search_dirs().into_iter().find_map(|dir| {
        extensions
            .iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|candidate| candidate.is_file())
    })
}

fn stored_key(kind: AiKind) -> Option<String> {
    kind.key()?;
    secrets::load(&kind.secret_id()).ok()
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn parse_version(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|word| word.trim_start_matches('v'))
        .find(|word| word.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(str::to_owned)
}

async fn version_of(program: &str) -> Option<String> {
    let output = capture(program, &["--version"]).await.ok()?;
    parse_version(&String::from_utf8_lossy(&output.stdout))
}

struct Probe {
    status: AiStatus,
    account: Option<String>,
    plan: Option<String>,
    detail: Option<String>,
    expires_at: Option<String>,
}

impl Probe {
    fn new(status: AiStatus) -> Self {
        Probe {
            status,
            account: None,
            plan: None,
            detail: None,
            expires_at: None,
        }
    }

    fn error(message: String) -> Self {
        Probe {
            detail: Some(message),
            ..Probe::new(AiStatus::Error)
        }
    }
}

pub(crate) fn claude_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| home().map(|home| home.join(".claude")))
}

async fn probe_claude(program: &str) -> Probe {
    let output = match capture(program, &["auth", "status"]).await {
        Ok(output) => output,
        Err(message) => return Probe::error(message),
    };
    let parsed = serde_json::from_slice::<Value>(&output.stdout).ok();
    let Some(json) = parsed.filter(|json| json.get("loggedIn").is_some()) else {
        let credentials = claude_dir().map(|dir| dir.join(".credentials.json"));
        return if credentials.is_some_and(|path| path.is_file()) {
            Probe::new(AiStatus::SignedIn)
        } else {
            Probe::new(AiStatus::SignedOut)
        };
    };
    if json["loggedIn"].as_bool() != Some(true) {
        return Probe::new(AiStatus::SignedOut);
    }
    let text = |key: &str| {
        json[key]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    Probe {
        account: text("email"),
        plan: text("subscriptionType")
            .map(|plan| capitalize(&plan))
            .or_else(|| text("authMethod")),
        expires_at: text("loginExpiresAt"),
        ..Probe::new(AiStatus::SignedIn)
    }
}

pub(crate) fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| home().map(|home| home.join(".codex")))
}

fn codex_identity() -> (Option<String>, Option<String>) {
    let Some(json) = codex_home().and_then(|dir| read_json(&dir.join("auth.json"))) else {
        return (None, None);
    };
    let claims = json["tokens"]["id_token"]
        .as_str()
        .and_then(|token| token.split('.').nth(1))
        .and_then(|payload| URL_SAFE_NO_PAD.decode(payload).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let Some(claims) = claims else {
        return (None, None);
    };
    let email = claims["email"].as_str().map(str::to_owned);
    let plan = claims["https://api.openai.com/auth"]["chatgpt_plan_type"]
        .as_str()
        .map(capitalize);
    (email, plan)
}

async fn probe_codex(program: &str) -> Probe {
    let output = match capture(program, &["login", "status"]).await {
        Ok(output) => output,
        Err(message) => return Probe::error(message),
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = text.to_lowercase();
    if lower.contains("not logged in") {
        return Probe::new(AiStatus::SignedOut);
    }
    if !output.status.success() || !lower.contains("logged in") {
        let line = text.lines().find(|line| !line.trim().is_empty());
        return Probe::error(
            line.map(|line| line.trim().to_owned())
                .unwrap_or_else(|| "codex login status завершился с ошибкой".into()),
        );
    }
    let (email, plan) = codex_identity();
    let method = if lower.contains("api key") {
        Some("API-ключ".to_owned())
    } else {
        None
    };
    Probe {
        account: email,
        plan: plan.or(method),
        ..Probe::new(AiStatus::SignedIn)
    }
}

fn gemini_dir() -> Option<PathBuf> {
    home().map(|home| home.join(".gemini"))
}

fn probe_gemini() -> Probe {
    let dir = gemini_dir();
    let oauth = dir
        .as_ref()
        .is_some_and(|dir| dir.join("oauth_creds.json").is_file());
    if oauth {
        let account = dir
            .and_then(|dir| read_json(&dir.join("google_accounts.json")))
            .and_then(|json| json["active"].as_str().map(str::to_owned));
        return Probe {
            account,
            plan: Some("Google".into()),
            ..Probe::new(AiStatus::SignedIn)
        };
    }
    let env_key = ["GEMINI_API_KEY", "GOOGLE_API_KEY"]
        .iter()
        .any(|name| std::env::var(name).is_ok_and(|value| !value.trim().is_empty()));
    if env_key {
        return Probe {
            plan: Some("API-ключ из окружения".into()),
            ..Probe::new(AiStatus::SignedIn)
        };
    }
    Probe::new(AiStatus::SignedOut)
}

async fn check_cli(kind: AiKind) -> AiView {
    let Some(spec) = kind.cli() else {
        return AiView::base(kind, AiStatus::Error);
    };
    let Some(path) = find_program(spec.binary) else {
        return AiView::base(kind, AiStatus::NotInstalled);
    };
    let program = path.to_string_lossy().into_owned();
    let (version, probe) = match kind {
        AiKind::Claude => tokio::join!(version_of(&program), probe_claude(&program)),
        AiKind::Codex => tokio::join!(version_of(&program), probe_codex(&program)),
        _ => (version_of(&program).await, probe_gemini()),
    };
    AiView {
        account: probe.account,
        plan: probe.plan,
        detail: probe.detail,
        version,
        expires_at: probe.expires_at,
        ..AiView::base(kind, probe.status)
    }
}

fn key_tail(key: &str) -> Option<String> {
    let count = key.chars().count();
    (count > 8).then(|| format!("ключ …{}", key.chars().skip(count - 4).collect::<String>()))
}

struct KeyInfo {
    plan: Option<String>,
}

async fn validate(kind: AiKind, key: &str) -> ApiResult<KeyInfo> {
    let spec = kind
        .key()
        .ok_or_else(|| ApiError::failed("этот провайдер входит через CLI"))?;
    let headers = match kind {
        AiKind::AnthropicApi => vec![
            ("x-api-key", key.to_owned()),
            ("anthropic-version", "2023-06-01".to_owned()),
        ],
        _ => vec![("Authorization", format!("Bearer {key}"))],
    };
    let json = Api::new(headers).json(spec.url, &[]).await?;
    let plan = match kind {
        AiKind::Openrouter => json["data"]["is_free_tier"].as_bool().map(|free| {
            if free {
                "бесплатный"
            } else {
                "платный"
            }
            .to_owned()
        }),
        _ => None,
    };
    Ok(KeyInfo { plan })
}

fn key_view(kind: AiKind, key: &str, result: ApiResult<KeyInfo>) -> AiView {
    match result {
        Ok(info) => AiView {
            account: key_tail(key),
            plan: info.plan,
            ..AiView::base(kind, AiStatus::SignedIn)
        },
        Err(ApiError::Rejected(message)) => AiView {
            detail: Some(message.replacen("токен", "ключ", 1)),
            ..AiView::base(kind, AiStatus::Rejected)
        },
        Err(ApiError::Unreachable(message)) => AiView {
            account: key_tail(key),
            detail: Some(message),
            ..AiView::base(kind, AiStatus::Unreachable)
        },
        Err(ApiError::Failed(message)) => AiView {
            account: key_tail(key),
            detail: Some(message),
            ..AiView::base(kind, AiStatus::Error)
        },
    }
}

async fn check_key(kind: AiKind) -> AiView {
    match stored_key(kind) {
        None => AiView::base(kind, AiStatus::SignedOut),
        Some(key) => {
            let result = validate(kind, &key).await;
            key_view(kind, &key, result)
        }
    }
}

async fn check_kind(kind: AiKind) -> AiView {
    if kind.cli().is_some() {
        check_cli(kind).await
    } else {
        check_key(kind).await
    }
}

#[cfg(windows)]
async fn open_console(command: &[&str]) -> Result<(), String> {
    let status = Command::new("cmd")
        .args(["/c", "start", "", "cmd", "/k"])
        .args(command)
        .creation_flags(0x0800_0000)
        .status()
        .await
        .map_err(|error| format!("не удалось открыть консоль: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("не удалось открыть консоль".into())
    }
}

#[cfg(target_os = "macos")]
async fn open_console(command: &[&str]) -> Result<(), String> {
    let script = format!(
        "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
        command.join(" ")
    );
    let status = Command::new("osascript")
        .args(["-e", &script])
        .status()
        .await
        .map_err(|error| format!("не удалось открыть Terminal: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("не удалось открыть Terminal".into())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
async fn open_console(command: &[&str]) -> Result<(), String> {
    let script = format!(
        "{}; echo; printf 'Нажмите Enter, чтобы закрыть: '; read -r _",
        command.join(" ")
    );
    for terminal in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
        let mut launcher = Command::new(terminal);
        launcher.arg(if terminal == "gnome-terminal" {
            "--"
        } else {
            "-e"
        });
        launcher.args(["sh", "-c", &script]);
        if let Ok(mut child) = launcher.spawn() {
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
            return Ok(());
        }
    }
    Err("не найден терминал для запуска входа".into())
}

fn cli_spec(kind: AiKind) -> Result<CliSpec, String> {
    kind.cli()
        .ok_or_else(|| format!("{}: вход по ключу, добавьте ключ", kind.label()))
}

fn ensure_installed(spec: &CliSpec) -> Result<(), String> {
    if find_program(spec.binary).is_some() {
        Ok(())
    } else {
        Err(format!("{} не установлен", spec.binary))
    }
}

pub async fn ai_list(state: &Connectors) -> Result<Vec<AiView>, String> {
    Ok(ALL.iter().map(|kind| state.ai.snapshot(*kind)).collect())
}

pub async fn ai_check(state: &Connectors) -> Result<Vec<AiView>, String> {
    let views = join_all(ALL.iter().map(|kind| check_kind(*kind))).await;
    for view in &views {
        state.ai.put(view);
    }
    state.publish();
    Ok(views)
}

pub async fn ai_refresh(state: &Connectors, kind: AiKind) -> Result<AiView, String> {
    let view = check_kind(kind).await;
    state.ai.put(&view);
    state.publish();
    Ok(view)
}

pub(super) fn snapshots(state: &Connectors) -> Vec<AiView> {
    ALL.iter().map(|kind| state.ai.snapshot(*kind)).collect()
}

fn health_of(status: AiStatus) -> Status {
    match status {
        AiStatus::SignedIn => Status::Ok,
        AiStatus::Unreachable => Status::Unreachable,
        AiStatus::Unchecked => Status::Unchecked,
        _ => Status::Error,
    }
}

pub(super) fn watched(state: &Connectors) -> Vec<(AiKind, Status)> {
    ALL.iter()
        .filter(|kind| kind.key().is_some())
        .map(|kind| (*kind, health_of(state.ai.snapshot(*kind).status)))
        .collect()
}

pub(super) async fn recheck(state: &Connectors, kind: AiKind) -> Status {
    let view = check_kind(kind).await;
    state.ai.put(&view);
    health_of(view.status)
}

pub async fn ai_login(state: &Connectors, kind: AiKind) -> Result<(), String> {
    let spec = cli_spec(kind)?;
    ensure_installed(&spec)?;
    state.ai.drop_kind(kind);
    open_console(spec.login).await
}

pub async fn ai_logout(state: &Connectors, kind: AiKind) -> Result<(), String> {
    let spec = cli_spec(kind)?;
    ensure_installed(&spec)?;
    state.ai.drop_kind(kind);
    match spec.logout {
        Some(command) => open_console(command).await,
        None => {
            let dir = gemini_dir().ok_or("не задан домашний каталог")?;
            let file = dir.join("oauth_creds.json");
            match std::fs::remove_file(&file) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("не удалось выйти из {}: {error}", kind.label())),
            }
        }
    }
}

pub async fn ai_key_set(state: &Connectors, kind: AiKind, key: String) -> Result<AiView, String> {
    if kind.key().is_none() {
        return Err(format!("{}: вход через CLI, ключ не нужен", kind.label()));
    }
    let key = key.trim().to_owned();
    if key.is_empty() {
        return Err("не указан ключ".into());
    }
    let info = validate(kind, &key).await.map_err(|error| match error {
        ApiError::Rejected(message) => message.replacen("токен", "ключ", 1),
        other => String::from(other),
    })?;
    secrets::save(&kind.secret_id(), &key)?;
    let view = key_view(kind, &key, Ok(info));
    state.ai.put(&view);
    Ok(view)
}

pub async fn ai_key_remove(state: &Connectors, kind: AiKind) -> Result<(), String> {
    if kind.key().is_none() {
        return Err(format!("{}: ключ не используется", kind.label()));
    }
    secrets::delete(&kind.secret_id());
    state.ai.drop_kind(kind);
    Ok(())
}

pub async fn ai_env(state: &Connectors) -> Result<HashMap<String, String>, String> {
    let mut env = HashMap::new();
    for kind in ALL {
        let Some(spec) = kind.key() else { continue };
        let Some(key) = stored_key(kind) else {
            continue;
        };
        if state
            .ai
            .get(kind)
            .is_some_and(|view| view.status == AiStatus::Rejected)
        {
            continue;
        }
        env.insert(spec.env.to_owned(), key);
    }
    Ok(env)
}
