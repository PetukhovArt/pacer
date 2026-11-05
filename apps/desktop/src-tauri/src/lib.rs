mod connectors;
mod files;
mod pacer;
mod settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(connectors::Connectors::load())
        .manage(pacer::Pacer::spawn())
        .manage(settings::SettingsWatch::default())
        .invoke_handler(tauri::generate_handler![
            pacer::pacer_watch,
            pacer::pacer_attach,
            pacer::pacer_input,
            pacer::pacer_resize,
            pacer::pacer_detach,
            pacer::pacer_mark_seen,
            pacer::pacer_call,
            settings::settings_read,
            settings::settings_write,
            settings::settings_watch,
            settings::settings_open_external,
            connectors::connectors_list,
            connectors::connectors_check,
            connectors::connectors_detect,
            connectors::connector_add,
            connectors::connector_update,
            connectors::connector_remove,
            connectors::connector_asset,
            connectors::forge::forge_inbox,
            connectors::forge::forge_for_worktree,
            connectors::forge::forge_change_request,
            connectors::forge::forge_diff,
            connectors::forge::forge_discussions,
            connectors::forge::forge_pipeline,
            connectors::forge::forge_file,
            files::fs_read_text,
            files::fs_read_binary,
            files::fs_watch,
            files::fs_unwatch,
            files::fs_list_markdown,
            files::fs_open_external,
            connectors::jira::jira_search,
            connectors::jira::jira_issue,
            connectors::confluence::confluence_search,
            connectors::confluence::confluence_page,
            connectors::ai::ai_list,
            connectors::ai::ai_check,
            connectors::ai::ai_refresh,
            connectors::ai::ai_login,
            connectors::ai::ai_logout,
            connectors::ai::ai_key_set,
            connectors::ai::ai_key_remove,
            connectors::ai::ai_env,
        ])
        .run(tauri::generate_context!())
        .expect("error while running veyra desktop");
}
