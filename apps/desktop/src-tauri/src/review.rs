use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tokio::process::Command;

const QUEUE_TIMEOUT: Duration = Duration::from_secs(60);
const CONFIG_FILES: [&str; 2] = [".jig/config.local.jsonc", ".jig/config.jsonc"];

#[derive(Debug, Serialize)]
pub struct ReviewRow {
    refs: Vec<String>,
    dir: String,
    number: u64,
    branch: String,
    draft: bool,
    approved: bool,
    url: String,
}

#[derive(Debug, Serialize)]
pub struct ReviewNote {
    refs: Vec<String>,
    text: String,
}

#[derive(Debug, Serialize)]
pub struct ReviewQueue {
    adapter: Option<String>,
    rows: Vec<ReviewRow>,
    notes: Vec<ReviewNote>,
}

fn strip_jsonc(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    let mut in_string = false;
    while index < chars.len() {
        let current = chars[index];
        if in_string {
            out.push(current);
            if current == '\\' && index + 1 < chars.len() {
                out.push(chars[index + 1]);
                index += 1;
            } else if current == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        match (current, chars.get(index + 1)) {
            ('"', _) => {
                in_string = true;
                out.push(current);
                index += 1;
            }
            ('/', Some('/')) => {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
            }
            ('/', Some('*')) => {
                index += 2;
                while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                    index += 1;
                }
                index += 2;
            }
            _ => {
                out.push(current);
                index += 1;
            }
        }
    }
    drop_trailing_commas(&out)
}

fn drop_trailing_commas(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut in_string = false;
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if in_string {
            out.push(current);
            if current == '\\' && index + 1 < chars.len() {
                out.push(chars[index + 1]);
                index += 1;
            } else if current == '"' {
                in_string = false;
            }
        } else if current == '"' {
            in_string = true;
            out.push(current);
        } else if current == ',' {
            let next = chars[index + 1..].iter().find(|c| !c.is_whitespace());
            if !matches!(next, Some('}') | Some(']')) {
                out.push(current);
            }
        } else {
            out.push(current);
        }
        index += 1;
    }
    out
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn adapter_setting(repo: &Path) -> Option<String> {
    CONFIG_FILES.iter().find_map(|file| {
        let text = std::fs::read_to_string(repo.join(file)).ok()?;
        let config: Value = serde_json::from_str(&strip_jsonc(&text)).ok()?;
        let adapter = config["reviewCycle"]["processAdapter"].as_str()?.trim();
        (!adapter.is_empty()).then(|| adapter.to_owned())
    })
}

fn resolve_adapter(repo: &Path, setting: &str) -> PathBuf {
    if let Some(rest) = setting.strip_prefix("~/") {
        return home().unwrap_or_default().join(rest);
    }
    let path = Path::new(setting);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo.join(path)
    }
}

#[cfg(windows)]
fn git_bash() -> Option<PathBuf> {
    if let Some(custom) = std::env::var_os("JIG_BASH") {
        return Some(PathBuf::from(custom));
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if !dir.join("git.exe").is_file() {
            continue;
        }
        let mut level = Some(dir.as_path());
        for _ in 0..3 {
            let Some(current) = level else { break };
            for candidate in [
                current.join("bin").join("bash.exe"),
                current.join("usr").join("bin").join("bash.exe"),
            ] {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            level = current.parent();
        }
    }
    None
}

#[cfg(windows)]
fn windows_path_with_bash(bash: &Path) -> Option<std::ffi::OsString> {
    let dir = bash.parent()?.to_path_buf();
    let mut entries = vec![dir];
    if let Some(existing) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(entries).ok()
}

fn command_for(adapter: &Path, repo: &Path) -> Result<Command, String> {
    let extension = adapter
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut command = match extension.as_str() {
        "mjs" | "js" | "cjs" => {
            let mut command = Command::new("node");
            command.arg(adapter);
            command
        }
        "sh" => {
            #[cfg(windows)]
            let program = git_bash().ok_or("Git Bash не найден: укажите JIG_BASH")?;
            #[cfg(not(windows))]
            let program = PathBuf::from("bash");
            let mut command = Command::new(program);
            command.arg(adapter);
            command
        }
        "ps1" => {
            let mut command = Command::new(if cfg!(windows) { "powershell" } else { "pwsh" });
            command
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(adapter);
            command
        }
        _ => Command::new(adapter),
    };
    #[cfg(windows)]
    if let Some(bash) = git_bash() {
        if let Some(path) = windows_path_with_bash(&bash) {
            command.env("PATH", path);
        }
    }
    command
        .arg("queue")
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    Ok(command)
}

fn failure(stderr: &str, code: Option<i32>) -> String {
    let line = stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let message = line.strip_prefix("ERROR").map_or(line, str::trim);
    if message.is_empty() {
        format!("адаптер завершился с кодом {}", code.unwrap_or(-1))
    } else {
        message.to_owned()
    }
}

fn refs_of(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_row(columns: &[&str]) -> Option<ReviewRow> {
    if columns.len() < 8 {
        return None;
    }
    Some(ReviewRow {
        refs: refs_of(columns[1]),
        dir: columns[2].to_owned(),
        number: columns[3].trim().parse().ok()?,
        branch: columns[4].to_owned(),
        draft: columns[5].trim() == "true",
        approved: columns[6].trim().eq_ignore_ascii_case("approved"),
        url: columns[7].trim().to_owned(),
    })
}

fn parse_queue(adapter: String, stdout: &str) -> ReviewQueue {
    let mut rows = Vec::new();
    let mut notes = Vec::new();
    for line in stdout.lines() {
        let columns: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        match columns[0] {
            "ROW" => match parse_row(&columns) {
                Some(row) => rows.push(row),
                None => notes.push(ReviewNote {
                    refs: Vec::new(),
                    text: format!("не разобрана строка адаптера: {line}"),
                }),
            },
            "NOTE" if columns.len() >= 3 => notes.push(ReviewNote {
                refs: refs_of(columns[1]),
                text: columns[2..].join(" "),
            }),
            _ => {}
        }
    }
    ReviewQueue {
        adapter: Some(adapter),
        rows,
        notes,
    }
}

pub async fn review_queue(repo: String) -> Result<ReviewQueue, String> {
    let repo = PathBuf::from(repo);
    let setting = if repo.is_dir() {
        adapter_setting(&repo)
    } else {
        None
    };
    let Some(setting) = setting else {
        return Ok(ReviewQueue {
            adapter: None,
            rows: Vec::new(),
            notes: Vec::new(),
        });
    };
    let adapter = resolve_adapter(&repo, &setting);
    if !adapter.is_file() {
        return Err(format!("адаптер не найден: {}", adapter.display()));
    }
    let mut command = command_for(&adapter, &repo)?;
    let run = async { crate::child::spawn(&mut command)?.wait_with_output().await };
    let output = match tokio::time::timeout(QUEUE_TIMEOUT, run).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("не найден интерпретатор для {}", adapter.display()))
        }
        Ok(Err(error)) => return Err(format!("не удалось запустить адаптер: {error}")),
        Err(_) => {
            return Err(format!(
                "адаптер не ответил за {} с",
                QUEUE_TIMEOUT.as_secs()
            ))
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match output.status.code() {
        Some(0) => Ok(parse_queue(setting, &stdout)),
        code => Err(failure(&stderr, code)),
    }
}

pub async fn review_adapter(repo: String) -> Result<Option<String>, String> {
    let repo = PathBuf::from(repo);
    let Some(setting) = repo.is_dir().then(|| adapter_setting(&repo)).flatten() else {
        return Ok(None);
    };
    let adapter = resolve_adapter(&repo, &setting);
    std::fs::canonicalize(&adapter)
        .map(|real| Some(real.to_string_lossy().into_owned()))
        .map_err(|_| format!("адаптер не найден: {}", adapter.display()))
}
