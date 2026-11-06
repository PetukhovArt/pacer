use std::path::PathBuf;

pub const DB_FILE: &str = "pacer.db";
pub const HOOK_TAG: &str = "_pacerManaged";
pub const CURSOR_TITLE_RULE: &str = "pacer-title.mdc";
pub const AGENT_ID: &str = "PACER_AGENT_ID";
pub const API_URL: &str = "PACER_API_URL";
pub const API_TOKEN: &str = "PACER_API_TOKEN";
pub const RUNTIME_DIR: &str = "PACER_RUNTIME_DIR";
pub const DATA_DIR: &str = "PACER_DATA_DIR";

pub const AGENT_SESSION_VARS: &[&str] = &[AGENT_ID, API_URL, API_TOKEN];

pub fn env_name(name: &str) -> Option<String> {
    use crate::env::{
        AGENT_CMD, CLOUD_MIRROR_SECS, EDITOR, IDLE_REAP_MS, INSTALL_URL, LOG, METRICS_SAMPLE_MS,
        WORKTREE_SYNC_MS,
    };
    const FALLBACK: &[&str] = &[
        AGENT_CMD,
        IDLE_REAP_MS,
        WORKTREE_SYNC_MS,
        METRICS_SAMPLE_MS,
        CLOUD_MIRROR_SECS,
        LOG,
        INSTALL_URL,
        EDITOR,
    ];
    FALLBACK
        .contains(&name)
        .then(|| name.replacen("BERTH_", "PACER_", 1))
}

pub fn default_data_dir() -> PathBuf {
    directories::ProjectDirs::from("dev", "", "pacer")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| crate::env::home_dir().unwrap_or_default().join(".pacer"))
}
