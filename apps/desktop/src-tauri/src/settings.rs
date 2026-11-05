use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::sink::BoxSink;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigFile {
    #[default]
    Settings,
    Keymap,
}

impl ConfigFile {
    fn file_name(self) -> &'static str {
        match self {
            Self::Settings => "settings.json",
            Self::Keymap => "keymap.json",
        }
    }

    fn temp_name(self) -> &'static str {
        match self {
            Self::Settings => "settings.json.tmp",
            Self::Keymap => "keymap.json.tmp",
        }
    }

    fn empty_text(self) -> &'static str {
        match self {
            Self::Settings => "{}\n",
            Self::Keymap => "[]\n",
        }
    }
}

#[derive(Default)]
pub struct SettingsWatch(Mutex<HashMap<ConfigFile, JoinHandle<()>>>);

impl Drop for SettingsWatch {
    fn drop(&mut self) {
        let slots = self.0.get_mut().unwrap_or_else(|e| e.into_inner());
        for (_, task) in slots.drain() {
            task.abort();
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SettingsChanged {
    text: String,
}

#[derive(Debug, PartialEq, Eq)]
struct Signature {
    modified: Option<SystemTime>,
    len: u64,
}

pub fn veyra_dir() -> Result<PathBuf, String> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(|home| PathBuf::from(home).join(".veyra"))
        .ok_or_else(|| format!("не задана переменная {var}"))
}

fn config_path(file: ConfigFile) -> Result<PathBuf, String> {
    Ok(veyra_dir()?.join(file.file_name()))
}

fn read_text(path: &Path) -> io::Result<String> {
    match fs::read_to_string(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        other => other,
    }
}

pub fn settings_text() -> String {
    config_path(ConfigFile::Settings)
        .ok()
        .and_then(|path| read_text(&path).ok())
        .unwrap_or_default()
}

fn signature(path: &Path) -> Option<Signature> {
    let meta = fs::metadata(path).ok()?;
    Some(Signature {
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

fn write_atomic(dir: &Path, file: ConfigFile, text: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let temp = dir.join(file.temp_name());
    fs::write(&temp, text.as_bytes())?;
    fs::rename(&temp, dir.join(file.file_name())).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

pub async fn settings_read(file: Option<ConfigFile>) -> Result<String, String> {
    let path = config_path(file.unwrap_or_default())?;
    read_text(&path).map_err(|e| format!("не удалось прочитать {}: {e}", path.display()))
}

pub async fn settings_write(text: String, file: Option<ConfigFile>) -> Result<(), String> {
    let file = file.unwrap_or_default();
    let dir = veyra_dir()?;
    write_atomic(&dir, file, &text).map_err(|e| {
        format!(
            "не удалось записать {}: {e}",
            dir.join(file.file_name()).display()
        )
    })
}

pub fn settings_watch(
    watch: &SettingsWatch,
    on_event: BoxSink<SettingsChanged>,
    file: Option<ConfigFile>,
) {
    let file = file.unwrap_or_default();
    let task = tokio::spawn(async move {
        let Ok(path) = config_path(file) else { return };
        let mut seen = signature(&path);
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let current = signature(&path);
            if current == seen {
                continue;
            }
            let Ok(text) = read_text(&path) else { continue };
            seen = current;
            if !on_event.send(SettingsChanged { text }) {
                return;
            }
        }
    });
    let mut slot = watch.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(previous) = slot.insert(file, task) {
        previous.abort();
    }
}

pub async fn settings_open_external(file: Option<ConfigFile>) -> Result<(), String> {
    let file = file.unwrap_or_default();
    let dir = veyra_dir()?;
    let path = dir.join(file.file_name());
    if !path.exists() {
        write_atomic(&dir, file, file.empty_text())
            .map_err(|e| format!("не удалось создать {}: {e}", path.display()))?;
    }
    open_in_default_app(&path).map_err(|e| format!("не удалось открыть редактор: {e}"))
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
