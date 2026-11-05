use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use tokio::task::{spawn_blocking, JoinHandle};

use crate::sink::BoxSink;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const TEXT_LIMIT: u64 = 20 * 1024 * 1024;
const BINARY_LIMIT: u64 = 25 * 1024 * 1024;
const LIST_LIMIT: usize = 2000;
const DIRS_LIMIT: usize = 500;
const SKIPPED_DIRS: [&str; 4] = ["node_modules", ".git", "target", "dist"];
const MARKDOWN_EXTENSIONS: [&str; 3] = ["md", "markdown", "mdx"];
const EXECUTABLE_EXTENSIONS: [&str; 24] = [
    "exe", "bat", "cmd", "com", "ps1", "psm1", "msi", "scr", "vbs", "vbe", "js", "jse", "wsf",
    "wsh", "lnk", "hta", "reg", "jar", "sh", "bash", "command", "app", "pif", "cpl",
];

static NEXT_WATCH: AtomicU32 = AtomicU32::new(1);

#[derive(Default)]
pub struct FileWatches(Mutex<HashMap<u32, JoinHandle<()>>>);

impl FileWatches {
    fn slots(&self) -> std::sync::MutexGuard<'_, HashMap<u32, JoinHandle<()>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for FileWatches {
    fn drop(&mut self) {
        for (_, task) in self.slots().drain() {
            task.abort();
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BinaryFile {
    mime: String,
    base64: String,
}

#[derive(Debug, Serialize)]
pub struct FileChanged {
    text: String,
}

#[derive(Debug, PartialEq, Eq)]
struct Signature {
    modified: Option<SystemTime>,
    len: u64,
}

fn signature(path: &Path) -> Option<Signature> {
    let meta = fs::metadata(path).ok()?;
    Some(Signature {
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let describe = |error: io::Error| format!("не удалось прочитать {}: {error}", path.display());
    let len = fs::metadata(path).map_err(describe)?.len();
    if len > limit {
        return Err(format!("файл слишком большой: {}", path.display()));
    }
    fs::read(path).map_err(describe)
}

fn read_text(path: &Path) -> Result<String, String> {
    let bytes = read_limited(path, TEXT_LIMIT)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

fn mime_of(path: &Path) -> &'static str {
    match extension(path).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

pub async fn fs_read_text(path: String) -> Result<String, String> {
    spawn_blocking(move || read_text(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}

pub async fn fs_read_binary(path: String) -> Result<BinaryFile, String> {
    spawn_blocking(move || {
        let path = PathBuf::from(path);
        let bytes = read_limited(&path, BINARY_LIMIT)?;
        Ok(BinaryFile {
            mime: mime_of(&path).to_owned(),
            base64: STANDARD.encode(bytes),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn fs_watch(watches: &FileWatches, path: String, on_event: BoxSink<FileChanged>) -> u32 {
    let id = NEXT_WATCH.fetch_add(1, Ordering::Relaxed);
    let task = tokio::spawn(async move {
        let path = PathBuf::from(path);
        let mut seen = signature(&path);
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let current = signature(&path);
            if current == seen {
                continue;
            }
            let Ok(text) = read_text(&path) else { continue };
            seen = current;
            if !on_event.send(FileChanged { text }) {
                return;
            }
        }
    });
    watches.slots().insert(id, task);
    id
}

pub fn fs_unwatch(watches: &FileWatches, id: u32) {
    let task = watches.slots().remove(&id);
    if let Some(task) = task {
        task.abort();
    }
}

fn collect_markdown(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    'walk: while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                let name = entry.file_name();
                if !SKIPPED_DIRS.iter().any(|skipped| name == *skipped) {
                    pending.push(path);
                }
            } else if kind.is_file() && MARKDOWN_EXTENSIONS.contains(&extension(&path).as_str()) {
                found.push(path.to_string_lossy().replace('\\', "/"));
                if found.len() >= LIST_LIMIT {
                    break 'walk;
                }
            }
        }
    }
    found.sort();
    found
}

pub async fn fs_list_markdown(root: String) -> Result<Vec<String>, String> {
    spawn_blocking(move || {
        let root = PathBuf::from(root);
        if !root.is_dir() {
            return Err(format!("не папка: {}", root.display()));
        }
        Ok(collect_markdown(&root))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn collect_dirs(root: &Path) -> Result<Vec<String>, String> {
    let entries =
        fs::read_dir(root).map_err(|e| format!("не удалось прочитать {}: {e}", root.display()))?;
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    names.truncate(DIRS_LIMIT);
    Ok(names)
}

pub async fn fs_list_dirs(root: String) -> Result<Vec<String>, String> {
    spawn_blocking(move || {
        let root = PathBuf::from(root);
        if !root.is_dir() {
            return Err(format!("не папка: {}", root.display()));
        }
        collect_dirs(&root)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn fs_open_external(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!("файл не найден: {}", path.display()));
    }
    if EXECUTABLE_EXTENSIONS.contains(&extension(&path).as_str()) {
        return Err(format!(
            "исполняемый файл не открывается: {}",
            path.display()
        ));
    }
    open_in_default_app(&path).map_err(|e| format!("не удалось открыть {}: {e}", path.display()))
}

#[cfg(windows)]
fn open_in_default_app(path: &Path) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("cmd")
        .args(["/c", "start", ""])
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
}

#[cfg(target_os = "macos")]
fn open_in_default_app(path: &Path) -> io::Result<()> {
    Command::new("open").arg(path).spawn().map(drop)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_default_app(path: &Path) -> io::Result<()> {
    Command::new("xdg-open").arg(path).spawn().map(drop)
}
