use std::sync::Arc;

use pacer_core::{AgentId, SessionRef};
use serde_json::Value;
use tauri::ipc::{Channel, IpcResponse};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::agent_sessions::{self, AgentSessionWatch, AgentSessionsChanged};
use crate::commands::{plain_commands, to_json};
use crate::connectors::monitor::{self, ConnectorsChanged};
use crate::connectors::Connectors;
use crate::files::{self, FileChanged, FileWatches};
use crate::pacer::{self, Pacer, SessionEvent, WatchEvent};
use crate::remote::{Remote, RemoteStatus, WebAsset, WebSource};
use crate::settings::{self, ConfigFile, SettingsChanged, SettingsWatch};
use crate::sink::Sink;
use crate::tray;

struct ChannelSink<T: IpcResponse>(Channel<T>);

impl<T: IpcResponse + Send + Sync + 'static> Sink<T> for ChannelSink<T> {
    fn send(&self, event: T) -> bool {
        self.0.send(event).is_ok()
    }
}

fn sink<T: IpcResponse + Send + Sync + 'static>(channel: Channel<T>) -> Box<ChannelSink<T>> {
    Box::new(ChannelSink(channel))
}

macro_rules! tauri_wrappers {
    ($( $kind:ident $name:ident ( $( $arg:ident : $ty:ty ),* ) = $path:path ; )*) => {
        $( tauri_wrappers!(@one $kind $name ( $( $arg : $ty ),* ) $path); )*
    };
    (@one state $($rest:tt)*) => { tauri_wrappers!(@state $($rest)*); };
    (@one local_state $($rest:tt)*) => { tauri_wrappers!(@state $($rest)*); };
    (@one free $($rest:tt)*) => { tauri_wrappers!(@free $($rest)*); };
    (@one local_free $($rest:tt)*) => { tauri_wrappers!(@free $($rest)*); };
    (@state $name:ident ( $( $arg:ident : $ty:ty ),* ) $path:path) => {
        #[tauri::command]
        pub async fn $name(state: State<'_, Arc<Connectors>>, $( $arg : $ty ),*) -> Result<Value, String> {
            to_json($path(&state, $( $arg ),*).await)
        }
    };
    (@free $name:ident ( $( $arg:ident : $ty:ty ),* ) $path:path) => {
        #[tauri::command]
        pub async fn $name($( $arg : $ty ),*) -> Result<Value, String> {
            to_json($path($( $arg ),*).await)
        }
    };
}

plain_commands!(tauri_wrappers);

#[tauri::command]
pub fn pacer_watch(pacer: State<'_, Pacer>, on_event: Channel<WatchEvent>) {
    pacer::pacer_watch(&pacer, sink(on_event));
}

#[tauri::command]
pub fn pacer_attach(
    pacer: State<'_, Pacer>,
    session: SessionRef,
    cols: u16,
    rows: u16,
    on_event: Channel<SessionEvent>,
) {
    pacer::pacer_attach(&pacer, session, cols, rows, sink(on_event));
}

#[tauri::command]
pub fn pacer_input(pacer: State<'_, Pacer>, session: SessionRef, data: String) {
    pacer::pacer_input(&pacer, session, data);
}

#[tauri::command]
pub fn pacer_resize(pacer: State<'_, Pacer>, session: SessionRef, cols: u16, rows: u16) {
    pacer::pacer_resize(&pacer, session, cols, rows);
}

#[tauri::command]
pub fn pacer_detach(pacer: State<'_, Pacer>, session: SessionRef) {
    pacer::pacer_detach(&pacer, session);
}

#[tauri::command]
pub fn pacer_mark_seen(pacer: State<'_, Pacer>, agent: AgentId) {
    pacer::pacer_mark_seen(&pacer, agent);
}

#[tauri::command]
pub async fn pacer_call(pacer: State<'_, Pacer>, request: Value) -> Result<Value, String> {
    pacer::pacer_call(&pacer, request).await
}

#[tauri::command]
pub async fn settings_watch(
    watch: State<'_, SettingsWatch>,
    on_event: Channel<SettingsChanged>,
    file: Option<ConfigFile>,
) -> Result<(), String> {
    settings::settings_watch(&watch, sink(on_event), file);
    Ok(())
}

#[tauri::command]
pub async fn connectors_watch(
    state: State<'_, Arc<Connectors>>,
    on_event: Channel<ConnectorsChanged>,
) -> Result<(), String> {
    monitor::connectors_watch(&state, sink(on_event)).await
}

#[tauri::command]
pub async fn fs_watch(
    watches: State<'_, FileWatches>,
    path: String,
    on_event: Channel<FileChanged>,
) -> Result<u32, String> {
    Ok(files::fs_watch(&watches, path, sink(on_event)))
}

#[tauri::command]
pub async fn agent_session_watch(
    watch: State<'_, AgentSessionWatch>,
    since_days: Option<u32>,
    on_event: Channel<AgentSessionsChanged>,
) -> Result<(), String> {
    agent_sessions::agent_session_watch(&watch, since_days, sink(on_event));
    Ok(())
}

#[tauri::command]
pub fn fs_unwatch(watches: State<'_, FileWatches>, id: u32) {
    files::fs_unwatch(&watches, id);
}

#[tauri::command]
pub fn remote_status(remote: State<'_, Remote>) -> RemoteStatus {
    remote.status()
}

#[tauri::command]
pub async fn remote_regenerate_token(remote: State<'_, Remote>) -> Result<RemoteStatus, String> {
    remote.regenerate().await
}

pub fn web_source<R: Runtime>(app: &AppHandle<R>) -> WebSource {
    if tauri::is_dev() {
        if let Some(url) = app.config().build.dev_url.as_ref() {
            return WebSource::Proxy(url.to_string());
        }
    }
    let resolver = app.asset_resolver();
    WebSource::Assets(Arc::new(move |path: &str| {
        let path = if path == "/" { "/index.html" } else { path };
        resolver.get(path.to_owned()).map(|asset| WebAsset {
            bytes: asset.bytes().to_vec(),
            mime: asset.mime_type().to_owned(),
        })
    }))
}

pub fn run() {
    let connectors = Arc::new(Connectors::load());
    let pacer = tauri::async_runtime::block_on(async { Pacer::spawn() });
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(connectors.clone())
        .manage(pacer)
        .manage(SettingsWatch::default())
        .manage(FileWatches::default())
        .manage(AgentSessionWatch::default())
        .setup(move |app| {
            tauri::async_runtime::spawn(monitor::run(connectors.clone()));
            tauri::async_runtime::spawn(crate::stratum::sweep_once());
            let remote = Remote::new(connectors, web_source(app.handle()));
            let watcher = remote.clone();
            tauri::async_runtime::spawn(async move { watcher.spawn_watch() });
            app.manage(remote);
            tray::install(app.handle())?;
            Ok(())
        })
        .on_window_event(tray::on_window_event)
        .invoke_handler(tauri::generate_handler![
            pacer_watch,
            pacer_attach,
            pacer_input,
            pacer_resize,
            pacer_detach,
            pacer_mark_seen,
            pacer_call,
            settings_read,
            settings_write,
            pacer_idle_timeout_read,
            pacer_idle_timeout_write,
            settings_watch,
            settings_open_external,
            connectors_list,
            connectors_check,
            connectors_reconnect,
            connectors_watch,
            connectors_detect,
            connector_add,
            connector_update,
            connector_remove,
            connector_asset,
            forge_inbox,
            forge_for_worktree,
            forge_change_request,
            forge_diff,
            forge_discussions,
            forge_pipeline,
            forge_file,
            fs_read_text,
            review_queue,
            review_adapter,
            fs_read_doc,
            fs_write_text,
            fs_read_binary,
            fs_watch,
            fs_unwatch,
            fs_list_markdown,
            fs_list_dirs,
            fs_open_external,
            stratum_probe,
            stratum_visualize,
            stratum_progress,
            stratum_release,
            agent_sessions_list,
            agent_session_watch,
            agent_session_transcript,
            jira_search,
            jira_issue,
            confluence_search,
            confluence_page,
            ai_list,
            ai_check,
            ai_refresh,
            ai_login,
            ai_logout,
            ai_key_set,
            ai_key_remove,
            ai_env,
            remote_status,
            remote_regenerate_token,
            tray::tray_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building veyra desktop")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                crate::stratum::shutdown();
            }
        });
}
