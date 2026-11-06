use std::fs;
use std::io;
use std::path::Path;
#[cfg(feature = "desktop")]
use std::path::PathBuf;

pub const DIR_NAME: &str = ".veyra";
pub const KEYRING_SERVICE: &str = "veyra";
#[cfg(feature = "desktop")]
pub const IDENTIFIER: &str = "dev.veyra.desktop";
const NOT_COPIED: &[&str] = &["stratum"];

pub fn env(name: &str) -> Option<String> {
    let read = |name: &str| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    read(name).or_else(|| {
        name.strip_prefix("BERTH_")
            .and_then(|rest| read(&format!("VEYRA_{rest}")))
    })
}

pub fn adopt_dir(old: &Path, new: &Path) {
    if new.exists() || !old.is_dir() {
        return;
    }
    let staging = new.with_extension("adopting");
    let _ = fs::remove_dir_all(&staging);
    let copied = copy_tree(old, &staging, NOT_COPIED).and_then(|()| fs::rename(&staging, new));
    if let Err(error) = copied {
        let _ = fs::remove_dir_all(&staging);
        eprintln!(
            "berth: could not copy {} to {}: {error}",
            old.display(),
            new.display()
        );
    }
}

#[cfg(feature = "desktop")]
pub fn adopt_webview_profile() {
    for base in webview_bases() {
        let (old, new) = (base.join(IDENTIFIER), base.join(crate::IDENTIFIER));
        let used = fs::read_dir(&new).is_ok_and(|mut entries| entries.next().is_some());
        if used || !old.is_dir() {
            continue;
        }
        let _ = copy_tree_lenient(&old, &new);
    }
}

#[cfg(feature = "desktop")]
fn webview_bases() -> Vec<PathBuf> {
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .into_iter()
            .collect()
    } else if cfg!(target_os = "macos") {
        home.map(|home| home.join("Library/WebKit"))
            .into_iter()
            .collect()
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|home| home.join(".local/share")))
            .into_iter()
            .collect()
    }
}

fn copy_tree(from: &Path, to: &Path, skip: &[&str]) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if skip.iter().any(|skipped| name == *skipped) {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target, &[])?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(feature = "desktop")]
fn copy_tree_lenient(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => {
                let _ = copy_tree_lenient(&entry.path(), &target);
            }
            Ok(_) => {
                let _ = fs::copy(entry.path(), target);
            }
            Err(_) => {}
        }
    }
    Ok(())
}
