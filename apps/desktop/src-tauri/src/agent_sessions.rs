use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::task::{spawn_blocking, JoinHandle};

use crate::connectors::ai::{claude_dir, codex_home};
use crate::sink::BoxSink;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const DEFAULT_DAYS: u32 = 14;
const MAX_DAYS: u32 = 365;
const DAY_MS: u64 = 86_400_000;
const HEAD_LINES: usize = 50;
const HEAD_BYTES: u64 = 512 * 1024;
const TAIL_BYTES: u64 = 64 * 1024;
const TRANSCRIPT_BYTES: u64 = 256 * 1024;
const TRANSCRIPT_MAX_BYTES: u64 = 16 * 1024 * 1024;
const TRANSCRIPT_DEFAULT: usize = 200;
const TRANSCRIPT_MAX: usize = 1000;
const SESSION_LIMIT: usize = 500;
const FRESH_MS: u64 = 10_000;
const IDLE_MS: u64 = 4 * 3_600_000;
const START_SLACK_MS: u64 = 5_000;
const TITLE_CHARS: usize = 120;
const TEXT_CHARS: usize = 20_000;
const TOOL_CHARS: usize = 240;
const HOST_DEPTH: usize = 8;
const ID_CHARS: usize = 100;
const TOOL_KEYS: [&str; 11] = [
    "command",
    "cmd",
    "file_path",
    "path",
    "pattern",
    "url",
    "query",
    "description",
    "prompt",
    "skill",
    "subject",
];
const CLAUDE_BATCH: [&str; 13] = [
    "-p",
    "--print",
    "mcp",
    "doctor",
    "update",
    "install",
    "setup-token",
    "config",
    "plugin",
    "-v",
    "--version",
    "-h",
    "--help",
];
const CODEX_BATCH: [&str; 16] = [
    "exec",
    "e",
    "mcp",
    "mcp-server",
    "app-server",
    "login",
    "logout",
    "completion",
    "apply",
    "a",
    "proto",
    "debug",
    "-V",
    "--version",
    "-h",
    "--help",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Harness {
    Claude,
    Codex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Working,
    Waiting,
    Idle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AgentSession {
    harness: Harness,
    id: String,
    cwd: String,
    title: String,
    branch: Option<String>,
    updated_at: u64,
    status: Status,
    running: bool,
    host: Option<String>,
    in_berth: bool,
    berth_pids: Vec<u32>,
    messages: u32,
}

#[derive(Debug, Serialize)]
pub struct AgentSessionsChanged {
    sessions: Vec<AgentSession>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Serialize)]
pub struct TranscriptMessage {
    key: String,
    role: Role,
    text: String,
    tool: Option<String>,
    at: Option<String>,
}

#[derive(Default)]
pub struct AgentSessionWatch(Mutex<Option<JoinHandle<()>>>);

impl Drop for AgentSessionWatch {
    fn drop(&mut self) {
        let slot = self.0.get_mut().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = slot.take() {
            task.abort();
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Last {
    #[default]
    Unknown,
    Working,
    Waiting,
}

#[derive(Clone, Debug, Default)]
struct Head {
    id: Option<String>,
    cwd: Option<String>,
    branch: Option<String>,
    custom: Option<String>,
    ai: Option<String>,
    summary: Option<String>,
    prompt: Option<String>,
    automated: bool,
}

#[derive(Clone, Debug, Default)]
struct Tail {
    last: Last,
    custom: Option<String>,
    ai: Option<String>,
    branch: Option<String>,
    messages: Option<u32>,
    counted: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Signature {
    modified: u64,
    len: u64,
}

struct Entry {
    harness: Harness,
    id: String,
    cwd_key: Option<String>,
    signature: Signature,
    head: Head,
    tail: Tail,
}

struct Found {
    harness: Harness,
    path: PathBuf,
    signature: Signature,
}

struct Proc {
    harness: Harness,
    cwd: Option<String>,
    resume: Option<String>,
    started: u64,
    host: Option<String>,
    berth: bool,
    berth_pids: Vec<u32>,
}

struct Claim {
    host: Option<String>,
    berth: bool,
    berth_pids: Vec<u32>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn cache() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

fn system() -> &'static Mutex<System> {
    static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();
    SYSTEM.get_or_init(|| Mutex::new(System::new()))
}

fn millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn now_ms() -> u64 {
    millis(SystemTime::now())
}

fn signature_of(meta: &fs::Metadata) -> Signature {
    Signature {
        modified: meta.modified().map(millis).unwrap_or(0),
        len: meta.len(),
    }
}

fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    path.trim_end_matches('/').to_lowercase()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ID_CHARS
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn owned(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut clipped: String = text.chars().take(max.saturating_sub(1)).collect();
    clipped.push('…');
    clipped
}

fn single_line(text: &str, max: usize) -> String {
    clip(&text.split_whitespace().collect::<Vec<_>>().join(" "), max)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn claude_root() -> Option<PathBuf> {
    claude_dir().map(|dir| dir.join("projects"))
}

fn codex_root() -> Option<PathBuf> {
    codex_home().map(|dir| dir.join("sessions"))
}

fn subdirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry.path())))
        .collect()
}

fn jsonl_files(dir: &Path, harness: Harness, since: u64, out: &mut Vec<Found>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) != Some("jsonl") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let signature = signature_of(&meta);
        if signature.modified >= since {
            out.push(Found {
                harness,
                path,
                signature,
            });
        }
    }
}

fn codex_days(root: &Path, since_day: i64) -> Vec<PathBuf> {
    let number = |name: &str| name.parse::<i64>().ok();
    let mut days = Vec::new();
    for (year_name, year_dir) in subdirs(root) {
        let Some(year) = number(&year_name) else {
            continue;
        };
        if days_from_civil(year + 1, 1, 1) <= since_day {
            continue;
        }
        for (month_name, month_dir) in subdirs(&year_dir) {
            let Some(month) = number(&month_name) else {
                continue;
            };
            let next = if month >= 12 {
                days_from_civil(year + 1, 1, 1)
            } else {
                days_from_civil(year, month + 1, 1)
            };
            if next <= since_day {
                continue;
            }
            for (day_name, day_dir) in subdirs(&month_dir) {
                let Some(day) = number(&day_name) else {
                    continue;
                };
                if days_from_civil(year, month, day) >= since_day {
                    days.push(day_dir);
                }
            }
        }
    }
    days
}

fn scan(since: u64) -> Vec<Found> {
    let mut found = Vec::new();
    if let Some(root) = claude_root() {
        for (_, dir) in subdirs(&root) {
            jsonl_files(&dir, Harness::Claude, since, &mut found);
        }
    }
    if let Some(root) = codex_root() {
        let since_day = i64::try_from(since / DAY_MS).unwrap_or(0) - 1;
        for dir in codex_days(&root, since_day) {
            jsonl_files(&dir, Harness::Codex, since, &mut found);
        }
    }
    found
}

fn parse_lines(buffer: &[u8]) -> impl Iterator<Item = Value> + '_ {
    buffer
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
}

fn read_tail(path: &Path, budget: u64) -> Option<(Vec<u8>, bool)> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(budget);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buffer = Vec::new();
    file.take(budget).read_to_end(&mut buffer).ok()?;
    if start > 0 {
        let cut = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |at| at + 1);
        buffer.drain(..cut);
    }
    Some((buffer, start == 0))
}

fn content_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => Some(text.clone()),
        Value::Array(blocks) => blocks.iter().find_map(|block| {
            matches!(
                block["type"].as_str(),
                Some("text" | "input_text" | "output_text")
            )
            .then(|| block["text"].as_str().map(str::to_owned))
            .flatten()
        }),
        _ => None,
    }
}

fn tag_value<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].trim())
}

fn command_line(text: &str) -> Option<String> {
    let name = tag_value(text, "command-name")?;
    let args = tag_value(text, "command-args").unwrap_or_default();
    Some(if args.is_empty() {
        name.to_owned()
    } else {
        format!("{name} {args}")
    })
}

fn prompt_of(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.starts_with("Caveat:") {
        return None;
    }
    if text.starts_with('<') {
        let args = tag_value(text, "command-args").filter(|args| !args.is_empty())?;
        let name = tag_value(text, "command-name").unwrap_or_default();
        return Some(format!("{name} {args}").trim().to_owned());
    }
    Some(text.to_owned())
}

fn shown_user_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("<local-command-caveat>")
        || trimmed.starts_with("<local-command-stdout>")
        || trimmed.starts_with("<environment_context>")
        || trimmed.starts_with("<user_instructions>")
        || trimmed.starts_with("Caveat:")
    {
        return None;
    }
    if trimmed.starts_with("<command-name>") || trimmed.starts_with("<command-message>") {
        return command_line(trimmed);
    }
    Some(clip(trimmed, TEXT_CHARS))
}

fn claude_head(head: &mut Head, line: &Value) {
    let kind = line["type"].as_str().unwrap_or_default();
    match kind {
        "custom-title" => head.custom = head.custom.take().or_else(|| owned(&line["customTitle"])),
        "ai-title" => head.ai = head.ai.take().or_else(|| owned(&line["aiTitle"])),
        "summary" => head.summary = head.summary.take().or_else(|| owned(&line["summary"])),
        _ => {}
    }
    head.id = head.id.take().or_else(|| owned(&line["sessionId"]));
    head.cwd = head.cwd.take().or_else(|| owned(&line["cwd"]));
    head.branch = head.branch.take().or_else(|| owned(&line["gitBranch"]));
    if line["entrypoint"]
        .as_str()
        .is_some_and(|entry| entry.starts_with("sdk"))
    {
        head.automated = true;
    }
    if kind == "user" && head.prompt.is_none() && line["isMeta"].as_bool() != Some(true) {
        head.prompt = content_text(&line["message"]["content"]).and_then(|text| prompt_of(&text));
    }
}

fn claude_tail(tail: &mut Tail, line: &Value) {
    if let Some(branch) = owned(&line["gitBranch"]) {
        tail.branch = Some(branch);
    }
    match line["type"].as_str().unwrap_or_default() {
        "custom-title" => tail.custom = owned(&line["customTitle"]).or(tail.custom.take()),
        "ai-title" => tail.ai = owned(&line["aiTitle"]).or(tail.ai.take()),
        "system" => match line["subtype"].as_str() {
            Some("turn_duration") => {
                tail.last = Last::Waiting;
                if let Some(count) = line["messageCount"].as_u64() {
                    tail.messages = Some(u32::try_from(count).unwrap_or(u32::MAX));
                }
            }
            Some("stop_hook_summary") => tail.last = Last::Waiting,
            _ => {}
        },
        "assistant" => {
            tail.counted += 1;
            let blocks = line["message"]["content"].as_array();
            let has = |wanted: &str| {
                blocks.is_some_and(|blocks| {
                    blocks
                        .iter()
                        .any(|block| block["type"].as_str() == Some(wanted))
                })
            };
            if has("tool_use") {
                tail.last = Last::Working;
            } else if has("text") {
                tail.last = Last::Waiting;
            }
        }
        "user" if line["isMeta"].as_bool() != Some(true) => {
            tail.counted += 1;
            let content = &line["message"]["content"];
            let result = content.as_array().is_some_and(|blocks| {
                blocks
                    .iter()
                    .any(|block| block["type"].as_str() == Some("tool_result"))
            });
            if result {
                tail.last = Last::Working;
                return;
            }
            let text = content_text(content).unwrap_or_default();
            let text = text.trim_start();
            if text.starts_with("[Request interrupted")
                || text.starts_with("<local-command-stdout>")
            {
                tail.last = Last::Waiting;
            } else if !text.starts_with("<command-name>")
                && !text.starts_with("<local-command-caveat>")
            {
                tail.last = Last::Working;
            }
        }
        _ => {}
    }
}

fn codex_head(head: &mut Head, line: &Value) {
    let payload = &line["payload"];
    match (line["type"].as_str(), payload["type"].as_str()) {
        (Some("session_meta"), _) => {
            head.id = head.id.take().or_else(|| owned(&payload["id"]));
            head.cwd = head.cwd.take().or_else(|| owned(&payload["cwd"]));
            head.branch = head
                .branch
                .take()
                .or_else(|| owned(&payload["git"]["branch"]));
            let originator = payload["originator"].as_str().unwrap_or_default();
            if originator.contains("exec") || payload["source"].as_str() == Some("exec") {
                head.automated = true;
            }
        }
        (Some("turn_context"), _) => head.cwd = head.cwd.take().or_else(|| owned(&payload["cwd"])),
        (Some("event_msg"), Some("user_message")) if head.prompt.is_none() => {
            head.prompt = owned(&payload["message"]).and_then(|text| shown_user_text(&text));
        }
        (Some("response_item"), Some("message"))
            if head.prompt.is_none() && payload["role"].as_str() == Some("user") =>
        {
            head.prompt = content_text(&payload["content"]).and_then(|text| shown_user_text(&text));
        }
        _ => {}
    }
}

fn codex_tail(tail: &mut Tail, line: &Value) {
    let payload = &line["payload"];
    match (line["type"].as_str(), payload["type"].as_str()) {
        (Some("event_msg"), Some("task_complete" | "turn_aborted")) => tail.last = Last::Waiting,
        (Some("event_msg"), Some("task_started")) => tail.last = Last::Working,
        (Some("event_msg"), Some("user_message")) => {
            tail.counted += 1;
            tail.last = Last::Working;
        }
        (Some("event_msg"), Some("agent_message")) => {
            tail.counted += 1;
            tail.last = Last::Waiting;
        }
        (
            Some("response_item"),
            Some(
                "function_call"
                | "custom_tool_call"
                | "local_shell_call"
                | "web_search_call"
                | "function_call_output"
                | "custom_tool_call_output",
            ),
        ) => tail.last = Last::Working,
        _ => {}
    }
}

fn read_head(path: &Path, harness: Harness) -> Head {
    let mut head = Head::default();
    let Ok(file) = File::open(path) else {
        return head;
    };
    let mut reader = BufReader::new(file.take(HEAD_BYTES));
    let mut line = Vec::new();
    for _ in 0..HEAD_LINES {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(value) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        match harness {
            Harness::Claude => claude_head(&mut head, &value),
            Harness::Codex => codex_head(&mut head, &value),
        }
    }
    head
}

fn read_tail_facts(path: &Path, harness: Harness) -> Tail {
    let mut tail = Tail::default();
    let Some((buffer, _)) = read_tail(path, TAIL_BYTES) else {
        return tail;
    };
    for line in parse_lines(&buffer) {
        match harness {
            Harness::Claude => claude_tail(&mut tail, &line),
            Harness::Codex => codex_tail(&mut tail, &line),
        }
    }
    tail
}

fn id_of(harness: Harness, path: &Path, head: &Head) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    match harness {
        Harness::Claude => stem,
        Harness::Codex => head
            .id
            .clone()
            .filter(|id| valid_id(id))
            .unwrap_or_else(|| {
                let start = stem.len().saturating_sub(36);
                stem.get(start..).unwrap_or(&stem).to_owned()
            }),
    }
}

fn refresh_entry(cache: &mut HashMap<PathBuf, Entry>, found: &Found) {
    if let Some(entry) = cache.get_mut(&found.path) {
        if entry.signature != found.signature {
            entry.signature = found.signature;
            entry.tail = read_tail_facts(&found.path, found.harness);
        }
        return;
    }
    let head = read_head(&found.path, found.harness);
    let tail = read_tail_facts(&found.path, found.harness);
    cache.insert(
        found.path.clone(),
        Entry {
            harness: found.harness,
            id: id_of(found.harness, &found.path, &head),
            cwd_key: head.cwd.as_deref().map(normalize),
            signature: found.signature,
            head,
            tail,
        },
    );
}

fn stem_of(name: &OsStr) -> String {
    let name = name.to_string_lossy().to_ascii_lowercase();
    name.strip_suffix(".exe").map(str::to_owned).unwrap_or(name)
}

fn arg_harness(arg: &str) -> Option<Harness> {
    let arg = arg.replace('\\', "/").to_ascii_lowercase();
    let file = arg.rsplit('/').next().unwrap_or(&arg);
    let file = file
        .strip_suffix(".exe")
        .or_else(|| file.strip_suffix(".js"))
        .or_else(|| file.strip_suffix(".cmd"))
        .unwrap_or(file);
    if arg.contains("@anthropic-ai/claude-code") || file == "claude" {
        Some(Harness::Claude)
    } else if arg.contains("@openai/codex") || file == "codex" {
        Some(Harness::Codex)
    } else {
        None
    }
}

fn harness_of(process: &Process) -> Option<Harness> {
    match stem_of(process.name()).as_str() {
        "claude" => Some(Harness::Claude),
        "codex" => Some(Harness::Codex),
        _ => process
            .cmd()
            .iter()
            .skip(1)
            .take(2)
            .find_map(|arg| arg_harness(&arg.to_string_lossy())),
    }
}

fn harness_args(args: &[String]) -> &[String] {
    let at = args
        .iter()
        .position(|arg| arg_harness(arg).is_some())
        .map_or(1, |at| at + 1);
    args.get(at..).unwrap_or_default()
}

fn interactive(harness: Harness, args: &[String]) -> bool {
    match harness {
        Harness::Claude => !args
            .iter()
            .any(|arg| CLAUDE_BATCH.contains(&arg.as_str()) || arg.starts_with("--print=")),
        Harness::Codex => args
            .iter()
            .find(|arg| !arg.starts_with('-') || CODEX_BATCH.contains(&arg.as_str()))
            .is_none_or(|first| !CODEX_BATCH.contains(&first.as_str())),
    }
}

fn resume_id(harness: Harness, args: &[String]) -> Option<String> {
    let value_after = |flags: &[&str]| {
        args.iter().enumerate().find_map(|(at, arg)| {
            if let Some((flag, value)) = arg.split_once('=') {
                if flags.contains(&flag) {
                    return Some(value.to_owned());
                }
            }
            if flags.contains(&arg.as_str()) {
                return args
                    .get(at + 1)
                    .filter(|next| !next.starts_with('-'))
                    .cloned();
            }
            None
        })
    };
    let id = match harness {
        Harness::Claude => value_after(&["--resume", "-r", "--session-id"]),
        Harness::Codex => value_after(&["resume"]),
    };
    id.filter(|id| valid_id(id))
}

fn host_label(name: &str) -> Option<&'static str> {
    let name = name.trim_end_matches("64");
    Some(match name {
        "windowsterminal" => "Windows Terminal",
        "code" | "code - insiders" => "VS Code",
        "cursor" => "Cursor",
        "windsurf" => "Windsurf",
        "zed" => "Zed",
        "webstorm" => "WebStorm",
        "idea" => "IntelliJ IDEA",
        "pycharm" => "PyCharm",
        "goland" => "GoLand",
        "rider" => "Rider",
        "clion" => "CLion",
        "rustrover" => "RustRover",
        "phpstorm" => "PhpStorm",
        "alacritty" => "Alacritty",
        "wezterm-gui" | "wezterm" => "WezTerm",
        "kitty" => "kitty",
        "gnome-terminal-server" => "GNOME Terminal",
        "konsole" => "Konsole",
        "iterm2" => "iTerm2",
        "terminal" => "Terminal",
        "warp" => "Warp",
        "tmux" | "tmux: server" => "tmux",
        "veyra-desktop" => "Veyra",
        _ => return None,
    })
}

fn shell_label(name: &str) -> Option<&'static str> {
    Some(match name {
        "pwsh" => "PowerShell",
        "powershell" => "Windows PowerShell",
        "cmd" => "Command Prompt",
        "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",
        _ => return None,
    })
}

fn host_of(system: &System, process: &Process) -> (Option<String>, bool, Vec<u32>) {
    let mut fallback = None;
    let mut chain = vec![process.pid().as_u32()];
    let mut current = process.parent();
    for _ in 0..HOST_DEPTH {
        let Some(parent) = current.and_then(|pid| system.process(pid)) else {
            break;
        };
        let name = stem_of(parent.name());
        if name.starts_with("berth") || name.starts_with("pacer") {
            return (Some("berth".to_owned()), true, chain);
        }
        if let Some(label) = host_label(&name) {
            return (Some(label.to_owned()), false, Vec::new());
        }
        chain.push(parent.pid().as_u32());
        fallback = fallback.or_else(|| shell_label(&name));
        current = parent.parent();
    }
    (fallback.map(str::to_owned), false, Vec::new())
}

fn processes() -> Vec<Proc> {
    let mut system = lock(system());
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let candidates: Vec<Pid> = system
        .processes()
        .iter()
        .filter(|(_, process)| {
            matches!(
                stem_of(process.name()).as_str(),
                "claude" | "codex" | "node" | "bun"
            )
        })
        .map(|(pid, _)| *pid)
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&candidates),
        false,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_cwd(UpdateKind::OnlyIfNotSet),
    );
    let harnesses: HashMap<Pid, Harness> = candidates
        .iter()
        .filter_map(|pid| Some((*pid, harness_of(system.process(*pid)?)?)))
        .collect();
    harnesses
        .iter()
        .filter_map(|(pid, harness)| {
            let process = system.process(*pid)?;
            if process.parent().and_then(|parent| harnesses.get(&parent)) == Some(harness) {
                return None;
            }
            let args: Vec<String> = process
                .cmd()
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let args = harness_args(&args);
            if !interactive(*harness, args) {
                return None;
            }
            let (host, berth, berth_pids) = host_of(&system, process);
            Some(Proc {
                harness: *harness,
                cwd: process
                    .cwd()
                    .map(|cwd| normalize(&cwd.to_string_lossy()))
                    .filter(|cwd| !cwd.is_empty()),
                resume: resume_id(*harness, args),
                started: process.start_time().saturating_mul(1000),
                host,
                berth,
                berth_pids,
            })
        })
        .collect()
}

fn claim(candidates: &[&Entry], procs: &[Proc]) -> HashMap<usize, Claim> {
    let mut claims = HashMap::new();
    let mut pending = Vec::new();
    for proc in procs {
        let exact = proc.resume.as_ref().and_then(|id| {
            candidates
                .iter()
                .position(|entry| entry.harness == proc.harness && &entry.id == id)
        });
        match exact {
            Some(at) => {
                claims.entry(at).or_insert(Claim {
                    host: proc.host.clone(),
                    berth: proc.berth,
                    berth_pids: proc.berth_pids.clone(),
                });
            }
            None => pending.push(proc),
        }
    }
    pending.sort_by_key(|proc| proc.cwd.is_none());
    for proc in pending {
        let pick = candidates.iter().enumerate().position(|(at, entry)| {
            !claims.contains_key(&at)
                && entry.harness == proc.harness
                && entry.signature.modified + START_SLACK_MS >= proc.started
                && proc
                    .cwd
                    .as_ref()
                    .is_none_or(|cwd| entry.cwd_key.as_ref() == Some(cwd))
        });
        if let Some(at) = pick {
            claims.insert(
                at,
                Claim {
                    host: proc.host.clone(),
                    berth: proc.berth,
                    berth_pids: proc.berth_pids.clone(),
                },
            );
        }
    }
    claims
}

fn title_of(entry: &Entry) -> String {
    [
        &entry.tail.custom,
        &entry.head.custom,
        &entry.tail.ai,
        &entry.head.ai,
        &entry.head.summary,
        &entry.head.prompt,
    ]
    .into_iter()
    .find_map(Option::as_deref)
    .map(|title| single_line(title, TITLE_CHARS))
    .unwrap_or_else(|| entry.id.chars().take(8).collect())
}

fn status_of(entry: &Entry, running: bool, now: u64) -> Status {
    let age = now.saturating_sub(entry.signature.modified);
    if age < FRESH_MS {
        return Status::Working;
    }
    if !running || age > IDLE_MS {
        return Status::Idle;
    }
    match entry.tail.last {
        Last::Working => Status::Working,
        Last::Waiting | Last::Unknown => Status::Waiting,
    }
}

fn session_of(entry: &Entry, claim: Option<&Claim>, now: u64) -> AgentSession {
    let running = claim.is_some();
    AgentSession {
        harness: entry.harness,
        id: entry.id.clone(),
        cwd: entry.head.cwd.clone().unwrap_or_default(),
        title: title_of(entry),
        branch: entry
            .tail
            .branch
            .clone()
            .or_else(|| entry.head.branch.clone()),
        updated_at: entry.signature.modified,
        status: status_of(entry, running, now),
        running,
        host: claim.and_then(|claim| claim.host.clone()),
        in_berth: claim.is_some_and(|claim| claim.berth),
        berth_pids: claim
            .map(|claim| claim.berth_pids.clone())
            .unwrap_or_default(),
        messages: entry.tail.messages.unwrap_or(entry.tail.counted),
    }
}

fn collect(since_days: Option<u32>) -> Vec<AgentSession> {
    let now = now_ms();
    let days = u64::from(since_days.unwrap_or(DEFAULT_DAYS).clamp(1, MAX_DAYS));
    let found = scan(now.saturating_sub(days * DAY_MS));
    let procs = processes();
    let mut cache = lock(cache());
    let seen: HashSet<&Path> = found.iter().map(|item| item.path.as_path()).collect();
    for item in &found {
        refresh_entry(&mut cache, item);
    }
    cache.retain(|path, _| seen.contains(path.as_path()) || path.exists());
    let mut candidates: Vec<&Entry> = found
        .iter()
        .filter_map(|item| cache.get(&item.path))
        .filter(|entry| !entry.head.automated && entry.cwd_key.is_some())
        .collect();
    candidates.sort_by_key(|entry| std::cmp::Reverse(entry.signature.modified));
    let mut ids = HashSet::new();
    candidates.retain(|entry| ids.insert((entry.harness, entry.id.as_str())));
    candidates.truncate(SESSION_LIMIT);
    let claims = claim(&candidates, &procs);
    candidates
        .iter()
        .enumerate()
        .map(|(at, entry)| session_of(entry, claims.get(&at), now))
        .collect()
}

fn locate(harness: Harness, id: &str) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err(format!("некорректный id сессии: {id}"));
    }
    let cached = lock(cache())
        .iter()
        .find(|(_, entry)| entry.harness == harness && entry.id == id)
        .map(|(path, _)| path.clone());
    if let Some(path) = cached {
        return Ok(path);
    }
    let missing = || format!("разговор не найден: {id}");
    match harness {
        Harness::Claude => {
            let root = claude_root().ok_or_else(missing)?;
            subdirs(&root)
                .into_iter()
                .map(|(_, dir)| dir.join(format!("{id}.jsonl")))
                .find(|path| path.is_file())
                .ok_or_else(missing)
        }
        Harness::Codex => {
            let root = codex_root().ok_or_else(missing)?;
            let suffix = format!("{id}.jsonl");
            codex_days(&root, i64::MIN)
                .into_iter()
                .flat_map(|dir| fs::read_dir(dir).into_iter().flatten().flatten())
                .map(|entry| entry.path())
                .find(|path| {
                    path.file_name()
                        .and_then(OsStr::to_str)
                        .is_some_and(|name| name.ends_with(&suffix))
                })
                .ok_or_else(missing)
        }
    }
}

fn tool_summary(input: &Value) -> String {
    let input = match input {
        Value::String(raw) => serde_json::from_str::<Value>(raw).unwrap_or_else(|_| input.clone()),
        _ => input.clone(),
    };
    if let Value::String(text) = &input {
        return single_line(text, TOOL_CHARS);
    }
    for key in TOOL_KEYS {
        let text = match &input[key] {
            Value::String(text) => text.clone(),
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
            _ => continue,
        };
        if !text.trim().is_empty() {
            return single_line(&text, TOOL_CHARS);
        }
    }
    if input.is_null() {
        return String::new();
    }
    single_line(&input.to_string(), TOOL_CHARS)
}

fn claude_messages(line: &Value, out: &mut Vec<TranscriptMessage>) {
    let role = match line["type"].as_str() {
        Some("user") => Role::User,
        Some("assistant") => Role::Assistant,
        _ => return,
    };
    if line["isMeta"].as_bool() == Some(true) || line["isSidechain"].as_bool() == Some(true) {
        return;
    }
    let uuid = line["uuid"].as_str().unwrap_or_default();
    let at = owned(&line["timestamp"]);
    let content = &line["message"]["content"];
    let blocks: Vec<&Value> = match content {
        Value::String(_) => vec![content],
        Value::Array(items) => items.iter().collect(),
        _ => return,
    };
    for (index, block) in blocks.into_iter().enumerate() {
        let key = format!("{uuid}:{index}");
        let text = block.as_str().or_else(|| {
            (block["type"].as_str() == Some("text"))
                .then(|| block["text"].as_str())
                .flatten()
        });
        if let Some(text) = text {
            let shown = match role {
                Role::User => shown_user_text(text),
                Role::Assistant => {
                    Some(clip(text.trim(), TEXT_CHARS)).filter(|text| !text.is_empty())
                }
            };
            if let Some(text) = shown {
                out.push(TranscriptMessage {
                    key,
                    role,
                    text,
                    tool: None,
                    at: at.clone(),
                });
            }
        } else if block["type"].as_str() == Some("tool_use") {
            out.push(TranscriptMessage {
                key,
                role: Role::Assistant,
                text: tool_summary(&block["input"]),
                tool: owned(&block["name"]).or_else(|| Some("tool".to_owned())),
                at: at.clone(),
            });
        }
    }
}

fn codex_messages(line: &Value, out: &mut Vec<TranscriptMessage>) {
    if line["type"].as_str() != Some("response_item") {
        return;
    }
    let payload = &line["payload"];
    let at = owned(&line["timestamp"]);
    let kind = payload["type"].as_str().unwrap_or_default();
    let key = format!(
        "{}:{kind}:{}:{}",
        at.as_deref().unwrap_or_default(),
        payload["call_id"].as_str().unwrap_or_default(),
        out.len()
    );
    let message = match kind {
        "message" => {
            let role = match payload["role"].as_str() {
                Some("user") => Role::User,
                Some("assistant") => Role::Assistant,
                _ => return,
            };
            let text = payload["content"]
                .as_array()
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter_map(|block| block["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            let shown = match role {
                Role::User => shown_user_text(&text),
                Role::Assistant => {
                    Some(clip(text.trim(), TEXT_CHARS)).filter(|text| !text.is_empty())
                }
            };
            let Some(text) = shown else {
                return;
            };
            TranscriptMessage {
                key,
                role,
                text,
                tool: None,
                at,
            }
        }
        "function_call" | "custom_tool_call" => TranscriptMessage {
            key,
            role: Role::Assistant,
            text: tool_summary(if kind == "function_call" {
                &payload["arguments"]
            } else {
                &payload["input"]
            }),
            tool: owned(&payload["name"]).or_else(|| Some("tool".to_owned())),
            at,
        },
        "local_shell_call" => TranscriptMessage {
            key,
            role: Role::Assistant,
            text: tool_summary(&payload["action"]),
            tool: Some("shell".to_owned()),
            at,
        },
        _ => return,
    };
    out.push(message);
}

fn read_transcript(
    harness: Harness,
    id: &str,
    tail: Option<u32>,
) -> Result<Vec<TranscriptMessage>, String> {
    let path = locate(harness, id)?;
    let wanted = tail
        .map_or(TRANSCRIPT_DEFAULT, |tail| tail as usize)
        .clamp(1, TRANSCRIPT_MAX);
    let mut budget = TRANSCRIPT_BYTES;
    loop {
        let (buffer, from_start) = read_tail(&path, budget)
            .ok_or_else(|| format!("не удалось прочитать {}", path.display()))?;
        let mut messages = Vec::new();
        for line in parse_lines(&buffer) {
            match harness {
                Harness::Claude => claude_messages(&line, &mut messages),
                Harness::Codex => codex_messages(&line, &mut messages),
            }
        }
        if messages.len() >= wanted || from_start || budget >= TRANSCRIPT_MAX_BYTES {
            let skip = messages.len().saturating_sub(wanted);
            messages.drain(..skip);
            return Ok(messages);
        }
        budget *= 4;
    }
}

pub async fn agent_sessions_list(since_days: Option<u32>) -> Result<Vec<AgentSession>, String> {
    spawn_blocking(move || collect(since_days))
        .await
        .map_err(|e| e.to_string())
}

pub async fn agent_session_transcript(
    harness: Harness,
    id: String,
    tail: Option<u32>,
) -> Result<Vec<TranscriptMessage>, String> {
    spawn_blocking(move || read_transcript(harness, &id, tail))
        .await
        .map_err(|e| e.to_string())?
}

pub fn agent_session_watch(
    watch: &AgentSessionWatch,
    since_days: Option<u32>,
    on_event: BoxSink<AgentSessionsChanged>,
) {
    if since_days == Some(0) {
        if let Some(previous) = lock(&watch.0).take() {
            previous.abort();
        }
        return;
    }
    let task = tokio::spawn(async move {
        let mut previous: Option<Vec<AgentSession>> = None;
        loop {
            if let Ok(sessions) = spawn_blocking(move || collect(since_days)).await {
                if previous.as_ref() != Some(&sessions) {
                    let event = AgentSessionsChanged {
                        sessions: sessions.clone(),
                    };
                    if !on_event.send(event) {
                        return;
                    }
                    previous = Some(sessions);
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
    if let Some(previous) = lock(&watch.0).replace(task) {
        previous.abort();
    }
}
