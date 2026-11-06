use anyhow::{Context, Result};
use berth_core::{crashlog, env, legacy, paths};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const STALE_STAGING: Duration = Duration::from_secs(3600);

pub fn run() -> Result<Option<String>> {
    let target = paths::data_dir();
    let Some(source) = source_dir() else {
        return Ok(None);
    };
    if target.exists() || !source.is_dir() || same_dir(&source, &target) {
        return Ok(None);
    }
    let staging = staging_dir(&target);
    remove_stale_staging(&target);
    if let Err(err) = copy_data_dir(&source, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(err.context(format!(
            "could not copy pacer data from {} to {}; nothing was changed, and creating {} starts berth without it",
            source.display(),
            target.display(),
            target.display()
        )));
    }
    if let Err(err) = std::fs::rename(&staging, &target) {
        let _ = std::fs::remove_dir_all(&staging);
        if target.exists() {
            return Ok(None);
        }
        return Err(anyhow::Error::new(err).context(format!(
            "could not move the copied pacer data into {}",
            target.display()
        )));
    }
    let note = format!(
        "copied pacer data from {} to {}; the pacer copy is left as it was",
        source.display(),
        target.display()
    );
    crashlog::append(&paths::log_dir().join("migration.log"), &note);
    Ok(Some(note))
}

fn source_dir() -> Option<PathBuf> {
    if let Some(dir) = env::non_empty(legacy::DATA_DIR) {
        return Some(PathBuf::from(dir));
    }
    env::non_empty(env::DATA_DIR)
        .is_none()
        .then(legacy::default_data_dir)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    paths::canonical_or_raw(a) == paths::canonical_or_raw(b)
}

fn staging_name(target: &Path) -> String {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "berth".into());
    format!(".{name}.migrating-")
}

fn staging_dir(target: &Path) -> PathBuf {
    target.with_file_name(format!("{}{}", staging_name(target), std::process::id()))
}

fn remove_stale_staging(target: &Path) {
    let prefix = staging_name(target);
    let Some(parent) = target.parent() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(&prefix) {
            continue;
        }
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > STALE_STAGING);
        if stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn copy_data_dir(source: &Path, staging: &Path) -> Result<()> {
    std::fs::create_dir_all(staging).with_context(|| format!("create {}", staging.display()))?;
    copy_tree(source, staging, true)?;
    let db = source.join(legacy::DB_FILE);
    if db.is_file() {
        let new_db = paths::db_path();
        let new_name = new_db
            .file_name()
            .context("database path has no file name")?;
        snapshot_db(&db, &staging.join(new_name))?;
    }
    Ok(())
}

fn skipped(name: &str, top_level: bool, is_dir: bool) -> bool {
    if top_level && is_dir && name == "state" {
        return true;
    }
    name == legacy::DB_FILE
        || name.starts_with(&format!("{}-", legacy::DB_FILE))
        || name.starts_with(&format!("{}.bak", legacy::DB_FILE))
        || name.ends_with(".pacer-tmp")
}

fn copy_tree(from: &Path, to: &Path, top_level: bool) -> Result<()> {
    for entry in std::fs::read_dir(from).with_context(|| format!("read {}", from.display()))? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        if skipped(&name.to_string_lossy(), top_level, kind.is_dir()) {
            continue;
        }
        let dest = to.join(&name);
        if kind.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_tree(&entry.path(), &dest, false)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), &dest)
                .with_context(|| format!("copy {}", entry.path().display()))?;
        }
    }
    Ok(())
}

fn snapshot_db(source: &Path, dest: &Path) -> Result<()> {
    use rusqlite::{Connection, OpenFlags};
    let wal = source.with_file_name(format!("{}-wal", legacy::DB_FILE));
    if !wal.exists() {
        std::fs::copy(source, dest).with_context(|| format!("copy {}", source.display()))?;
        return Ok(());
    }
    let conn = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open {} read-only", source.display()))?;
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])
        .with_context(|| format!("snapshot {} into {}", source.display(), dest.display()))?;
    Ok(())
}
