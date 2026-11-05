use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::Url;
use tokio::process::Command;

use super::http::{ApiError, ApiResult};
use super::types::Kind;

const TOKEN_TTL: Duration = Duration::from_secs(300);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

pub async fn capture(program: &str, args: &[&str]) -> Result<Output, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let run = command.output();
    match tokio::time::timeout(COMMAND_TIMEOUT, run).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("{program} не найден в PATH"))
        }
        Ok(Err(error)) => Err(format!("не удалось запустить {program}: {error}")),
        Err(_) => Err(format!(
            "{program} не ответил за {} с",
            COMMAND_TIMEOUT.as_secs()
        )),
    }
}

pub fn host_port(base_url: &str) -> Option<String> {
    let url = Url::parse(base_url).ok()?;
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

#[derive(Default)]
pub struct TokenCache(Mutex<HashMap<String, (Instant, String)>>);

impl TokenCache {
    fn fresh(&self, key: &str) -> Option<String> {
        let cache = self.0.lock().ok()?;
        let (at, token) = cache.get(key)?;
        (at.elapsed() < TOKEN_TTL).then(|| token.clone())
    }

    fn store(&self, key: String, token: String) {
        if let Ok(mut cache) = self.0.lock() {
            cache.insert(key, (Instant::now(), token));
        }
    }

    pub async fn token(&self, kind: Kind, base_url: &str) -> ApiResult<String> {
        let host = host_port(base_url)
            .ok_or_else(|| ApiError::failed(format!("некорректный адрес: {base_url}")))?;
        let key = format!("{kind:?}:{host}");
        if let Some(token) = self.fresh(&key) {
            return Ok(token);
        }
        let (program, args): (&str, Vec<&str>) = match kind {
            Kind::Github => ("gh", vec!["auth", "token", "--hostname", &host]),
            Kind::Gitlab => ("glab", vec!["config", "get", "token", "--host", &host]),
            _ => {
                return Err(ApiError::failed(format!(
                    "{}: вход через CLI не поддерживается, нужен токен",
                    kind.title()
                )))
            }
        };
        let output = capture(program, &args).await.map_err(ApiError::Failed)?;
        let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !output.status.success() || token.is_empty() {
            return Err(ApiError::Failed(format!(
                "{program} не авторизован для {host}: выполните {} login",
                program
            )));
        }
        self.store(key, token.clone());
        Ok(token)
    }
}

pub struct CliLogin {
    pub kind: Kind,
    pub base_url: String,
    pub account: Option<String>,
}

pub async fn detect() -> Vec<CliLogin> {
    let mut found = detect_github().await;
    found.extend(detect_gitlab());
    found
}

async fn detect_github() -> Vec<CliLogin> {
    let Ok(output) = capture("gh", &["auth", "status"]).await else {
        return Vec::new();
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut found: Vec<CliLogin> = Vec::new();
    for line in text.lines() {
        let Some((_, rest)) = line.split_once("Logged in to ") else {
            continue;
        };
        let mut words = rest.split_whitespace();
        let Some(host) = words.next() else { continue };
        let account = match words.next() {
            Some("account") | Some("as") => words.next().map(str::to_owned),
            _ => None,
        };
        let base_url = format!("https://{host}");
        if found.iter().all(|login| login.base_url != base_url) {
            found.push(CliLogin {
                kind: Kind::Github,
                base_url,
                account,
            });
        }
    }
    found
}

fn glab_config_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("GLAB_CONFIG_DIR") {
        candidates.push(PathBuf::from(dir).join("config.yml"));
    }
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(dir).join("glab-cli").join("config.yml"));
    }
    if let Some(dir) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(dir).join("glab-cli").join("config.yml"));
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    if let Some(home) = home {
        candidates.push(
            PathBuf::from(home)
                .join(".config")
                .join("glab-cli")
                .join("config.yml"),
        );
    }
    candidates.into_iter().find(|path| path.is_file())
}

struct GlabHost {
    name: String,
    protocol: String,
    has_token: bool,
}

fn parse_glab_hosts(text: &str) -> Vec<GlabHost> {
    let mut hosts: Vec<GlabHost> = Vec::new();
    let mut in_hosts = false;
    let mut host_indent: Option<usize> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            in_hosts = trimmed == "hosts:";
            host_indent = None;
            continue;
        }
        if !in_hosts {
            continue;
        }
        let level = *host_indent.get_or_insert(indent);
        if indent == level && trimmed.ends_with(':') {
            hosts.push(GlabHost {
                name: trimmed
                    .trim_end_matches(':')
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_owned(),
                protocol: "https".into(),
                has_token: false,
            });
        } else if indent > level {
            let (Some(host), Some((key, value))) = (hosts.last_mut(), trimmed.split_once(':'))
            else {
                continue;
            };
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            match key.trim() {
                "api_protocol" if !value.is_empty() => host.protocol = value.to_owned(),
                "token" => host.has_token = !value.is_empty() && !value.starts_with("!!null"),
                _ => {}
            }
        }
    }
    hosts
}

fn detect_gitlab() -> Vec<CliLogin> {
    let Some(path) = glab_config_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_glab_hosts(&text)
        .into_iter()
        .filter(|host| host.has_token)
        .map(|host| CliLogin {
            kind: Kind::Gitlab,
            base_url: format!("{}://{}", host.protocol, host.name),
            account: None,
        })
        .collect()
}
