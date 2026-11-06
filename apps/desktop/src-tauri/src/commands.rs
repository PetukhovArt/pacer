use serde::Serialize;
use serde_json::Value;

pub fn to_json<T: Serialize>(result: Result<T, String>) -> Result<Value, String> {
    result.and_then(|value| serde_json::to_value(value).map_err(|error| error.to_string()))
}

macro_rules! plain_commands {
    ($then:ident) => {
        $then! {
            state connectors_list() = $crate::connectors::connectors_list;
            state connectors_check() = $crate::connectors::connectors_check;
            state connectors_reconnect() = $crate::connectors::monitor::connectors_reconnect;
            state connectors_detect() = $crate::connectors::connectors_detect;
            state connector_add(
                kind: $crate::connectors::types::Kind,
                base_url: String,
                label: Option<String>,
                auth: $crate::connectors::types::AuthMode,
                token: Option<String>,
                email: Option<String>
            ) = $crate::connectors::connector_add;
            state connector_update(
                id: String,
                label: Option<String>,
                token: Option<String>,
                email: Option<String>
            ) = $crate::connectors::connector_update;
            state connector_remove(id: String) = $crate::connectors::connector_remove;
            state connector_asset(id: String, url: String) = $crate::connectors::connector_asset;
            state forge_inbox() = $crate::connectors::forge::forge_inbox;
            state forge_for_worktree(path: String, branch: String) = $crate::connectors::forge::forge_for_worktree;
            state forge_change_request(connector: String, project: String, number: u64) = $crate::connectors::forge::forge_change_request;
            state forge_diff(connector: String, project: String, number: u64) = $crate::connectors::forge::forge_diff;
            state forge_discussions(connector: String, project: String, number: u64) = $crate::connectors::forge::forge_discussions;
            state forge_pipeline(connector: String, project: String, number: u64) = $crate::connectors::forge::forge_pipeline;
            state forge_file(connector: String, project: String, r#ref: String, path: String) = $crate::connectors::forge::forge_file;
            state forge_source(connector: String, project: String, number: u64) = $crate::connectors::forge::forge_source;
            free mr_fetch_source(repo: String, branch: String, local: String, fork: Option<$crate::connectors::types::Fork>) = $crate::mr_checkout::mr_fetch_source;
            state jira_search(connector: String, jql: String, start: Option<u32>, limit: Option<u32>) = $crate::connectors::jira::jira_search;
            state jira_issue(connector: String, key: String) = $crate::connectors::jira::jira_issue;
            state confluence_search(connector: String, text: String, limit: Option<u32>) = $crate::connectors::confluence::confluence_search;
            state confluence_page(connector: String, id: String) = $crate::connectors::confluence::confluence_page;
            state ai_list() = $crate::connectors::ai::ai_list;
            state ai_check() = $crate::connectors::ai::ai_check;
            state ai_refresh(kind: $crate::connectors::ai::AiKind) = $crate::connectors::ai::ai_refresh;
            state ai_key_set(kind: $crate::connectors::ai::AiKind, key: String) = $crate::connectors::ai::ai_key_set;
            state ai_key_remove(kind: $crate::connectors::ai::AiKind) = $crate::connectors::ai::ai_key_remove;
            local_state ai_login(kind: $crate::connectors::ai::AiKind) = $crate::connectors::ai::ai_login;
            local_state ai_logout(kind: $crate::connectors::ai::AiKind) = $crate::connectors::ai::ai_logout;
            local_state ai_env() = $crate::connectors::ai::ai_env;
            free settings_read(file: Option<$crate::settings::ConfigFile>) = $crate::settings::settings_read;
            free settings_write(text: String, file: Option<$crate::settings::ConfigFile>) = $crate::settings::settings_write;
            free berth_idle_timeout_read() = $crate::settings::berth_idle_timeout_read;
            free berth_idle_timeout_write(value: String) = $crate::settings::berth_idle_timeout_write;
            free agent_sessions_list(since_days: Option<u32>) = $crate::agent_sessions::agent_sessions_list;
            free agent_session_transcript(harness: $crate::agent_sessions::Harness, id: String, tail: Option<u32>) = $crate::agent_sessions::agent_session_transcript;
            free review_queue(repo: String) = $crate::review::review_queue;
            free review_adapter(repo: String) = $crate::review::review_adapter;
            free fs_read_text(path: String) = $crate::files::fs_read_text;
            free fs_read_doc(path: String) = $crate::files::fs_read_doc;
            free fs_write_text(path: String, text: String, expected: String) = $crate::files::fs_write_text;
            free fs_read_binary(path: String) = $crate::files::fs_read_binary;
            free fs_list_markdown(root: String) = $crate::files::fs_list_markdown;
            free fs_list_dirs(root: String) = $crate::files::fs_list_dirs;
            local_free settings_open_external(file: Option<$crate::settings::ConfigFile>) = $crate::settings::settings_open_external;
            local_free fs_open_external(path: String) = $crate::files::fs_open_external;
            local_free stratum_probe(repo: String, target: String) = $crate::stratum::stratum_probe;
            local_state stratum_visualize(connector: String, project: String, number: u64, repo: String, target: String) = $crate::stratum::stratum_visualize;
            local_free stratum_progress(connector: String, project: String, number: u64) = $crate::stratum::stratum_progress;
            local_free stratum_release(key: String) = $crate::stratum::stratum_release;
        }
    };
}

pub(crate) use plain_commands;
