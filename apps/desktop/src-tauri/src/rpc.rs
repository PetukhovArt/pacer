use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use berth_core::{AgentId, Entity, SessionRef};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::Notify;

use crate::agent_sessions::{self, AgentSessionWatch};
use crate::berth::{self, Berth, WatchEvent};
use crate::commands::{plain_commands, to_json};
use crate::connectors::monitor;
use crate::connectors::Connectors;
use crate::files::{self, FileWatches};
use crate::settings::{self, ConfigFile, SettingsWatch};
use crate::sink::{BoxSink, Sink};

const GUARDED: [(&str, &str); 10] = [
    ("review_queue", "repo"),
    ("review_adapter", "repo"),
    ("fs_read_text", "path"),
    ("fs_read_doc", "path"),
    ("fs_write_text", "path"),
    ("fs_read_binary", "path"),
    ("fs_list_markdown", "root"),
    ("fs_list_dirs", "root"),
    ("fs_watch", "path"),
    ("forge_for_worktree", "path"),
];

fn bad_args(error: serde_json::Error) -> String {
    format!("некорректные аргументы: {error}")
}

fn refused(cmd: &str) -> String {
    format!("{cmd} недоступна при удалённом доступе")
}

macro_rules! rpc_dispatch {
    ($( $kind:ident $name:ident ( $( $arg:ident : $ty:ty ),* ) = $path:path ; )*) => {
        async fn plain(connectors: &Connectors, cmd: &str, args: Value) -> Option<Result<Value, String>> {
            match cmd {
                $( stringify!($name) => Some(rpc_dispatch!(@one $kind connectors args $name ( $( $arg : $ty ),* ) $path)), )*
                _ => None,
            }
        }
    };
    (@one state $c:ident $a:ident $name:ident ( $( $arg:ident : $ty:ty ),* ) $path:path) => {{
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args { $( $arg : $ty ),* }
        match serde_json::from_value::<Args>($a) {
            Ok(Args { $( $arg ),* }) => to_json($path($c, $( $arg ),*).await),
            Err(error) => Err(bad_args(error)),
        }
    }};
    (@one free $c:ident $a:ident $name:ident ( $( $arg:ident : $ty:ty ),* ) $path:path) => {{
        let _ = $c;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Args { $( $arg : $ty ),* }
        match serde_json::from_value::<Args>($a) {
            Ok(Args { $( $arg ),* }) => to_json($path($( $arg ),*).await),
            Err(error) => Err(bad_args(error)),
        }
    }};
    (@one local_state $c:ident $a:ident $name:ident $args:tt $path:path) => {{
        let _ = ($c, $a);
        Err(refused(stringify!($name)))
    }};
    (@one local_free $c:ident $a:ident $name:ident $args:tt $path:path) => {{
        let _ = ($c, $a);
        Err(refused(stringify!($name)))
    }};
}

plain_commands!(rpc_dispatch);

#[derive(Deserialize)]
struct ChannelRef {
    #[serde(rename = "__channel")]
    channel: u32,
}

pub const OUTBOX_CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct Outbox {
    queue: Sender<String>,
    overflow: Arc<Notify>,
}

impl Outbox {
    pub fn new() -> (Self, Receiver<String>) {
        let (queue, receiver) = mpsc::channel(OUTBOX_CAPACITY);
        let outbox = Self {
            queue,
            overflow: Arc::new(Notify::new()),
        };
        (outbox, receiver)
    }

    pub fn push(&self, frame: String) -> bool {
        match self.queue.try_send(frame) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.overflow.notify_one();
                false
            }
            Err(TrySendError::Closed(_)) => false,
        }
    }

    pub async fn overflowed(&self) {
        self.overflow.notified().await;
    }
}

struct WsSink {
    channel: u32,
    out: Outbox,
}

impl<T: Serialize + Send + 'static> Sink<T> for WsSink {
    fn send(&self, event: T) -> bool {
        let frame = json!({ "channel": self.channel, "message": event });
        self.out.push(frame.to_string())
    }
}

type Roots = Arc<Mutex<Vec<PathBuf>>>;

struct RootsSink {
    inner: WsSink,
    roots: Roots,
}

impl RootsSink {
    fn remember(&self, paths: impl IntoIterator<Item = PathBuf>) {
        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        for path in paths {
            if let Ok(path) = std::fs::canonicalize(&path) {
                if !roots.contains(&path) {
                    roots.push(path);
                }
            }
        }
    }
}

impl Sink<WatchEvent> for RootsSink {
    fn send(&self, event: WatchEvent) -> bool {
        match &event {
            WatchEvent::Snapshot {
                projects,
                worktrees,
                ..
            } => self.remember(
                projects
                    .iter()
                    .map(|project| project.repo_path.clone())
                    .chain(worktrees.iter().map(|worktree| worktree.path.clone())),
            ),
            WatchEvent::Upserted {
                entity: Entity::Worktree(worktree),
            } => self.remember([worktree.path.clone()]),
            WatchEvent::Upserted {
                entity: Entity::Project(project),
            } => self.remember([project.repo_path.clone()]),
            _ => {}
        }
        self.inner.send(event)
    }
}

pub struct Client {
    connectors: Arc<Connectors>,
    berth: Berth,
    settings: SettingsWatch,
    files: FileWatches,
    sessions: AgentSessionWatch,
    roots: Roots,
    out: Outbox,
}

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(bad_args)
}

fn inside(roots: &[PathBuf], raw: &str) -> bool {
    std::fs::canonicalize(Path::new(raw))
        .is_ok_and(|path| roots.iter().any(|root| path.starts_with(root)))
}

fn parent_of_root(roots: &[PathBuf], raw: &str) -> bool {
    std::fs::canonicalize(Path::new(raw)).is_ok_and(|path| {
        roots
            .iter()
            .any(|root| root.parent() == Some(path.as_path()))
    })
}

impl Client {
    pub fn new(connectors: Arc<Connectors>, out: Outbox) -> Self {
        Self {
            connectors,
            berth: Berth::spawn(),
            settings: SettingsWatch::default(),
            files: FileWatches::default(),
            sessions: AgentSessionWatch::default(),
            roots: Roots::default(),
            out,
        }
    }

    fn sink(&self, channel: u32) -> WsSink {
        WsSink {
            channel,
            out: self.out.clone(),
        }
    }

    fn guard(&self, cmd: &str, args: &Value) -> Result<(), String> {
        let Some((_, key)) = GUARDED.iter().find(|(name, _)| *name == cmd) else {
            return Ok(());
        };
        let raw = args.get(*key).and_then(Value::as_str).unwrap_or_default();
        let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
        let allowed = if cmd == "fs_list_dirs" {
            parent_of_root(&roots, raw)
        } else {
            inside(&roots, raw)
        };
        if allowed {
            Ok(())
        } else {
            Err(format!("путь вне рабочих деревьев berth: {raw}"))
        }
    }

    pub async fn call(&self, cmd: &str, args: Value) -> Result<Value, String> {
        let args = if args.is_null() { json!({}) } else { args };
        self.guard(cmd, &args)?;
        match cmd {
            "berth_watch" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Args {
                    on_event: ChannelRef,
                }
                let Args { on_event } = parse(args)?;
                let sink: BoxSink<WatchEvent> = Box::new(RootsSink {
                    inner: self.sink(on_event.channel),
                    roots: self.roots.clone(),
                });
                berth::berth_watch(&self.berth, sink);
                Ok(Value::Null)
            }
            "berth_attach" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Args {
                    session: SessionRef,
                    cols: u16,
                    rows: u16,
                    on_event: ChannelRef,
                }
                let Args {
                    session,
                    cols,
                    rows,
                    on_event,
                } = parse(args)?;
                let sink = Box::new(self.sink(on_event.channel));
                berth::berth_attach(&self.berth, session, cols, rows, sink);
                Ok(Value::Null)
            }
            "berth_input" => {
                #[derive(Deserialize)]
                struct Args {
                    session: SessionRef,
                    data: String,
                }
                let Args { session, data } = parse(args)?;
                berth::berth_input(&self.berth, session, data);
                Ok(Value::Null)
            }
            "berth_resize" => {
                #[derive(Deserialize)]
                struct Args {
                    session: SessionRef,
                    cols: u16,
                    rows: u16,
                }
                let Args {
                    session,
                    cols,
                    rows,
                } = parse(args)?;
                berth::berth_resize(&self.berth, session, cols, rows);
                Ok(Value::Null)
            }
            "berth_detach" => {
                #[derive(Deserialize)]
                struct Args {
                    session: SessionRef,
                }
                let Args { session } = parse(args)?;
                berth::berth_detach(&self.berth, session);
                Ok(Value::Null)
            }
            "berth_mark_seen" => {
                #[derive(Deserialize)]
                struct Args {
                    agent: AgentId,
                }
                let Args { agent } = parse(args)?;
                berth::berth_mark_seen(&self.berth, agent);
                Ok(Value::Null)
            }
            "berth_call" => {
                #[derive(Deserialize)]
                struct Args {
                    request: Value,
                }
                let Args { request } = parse(args)?;
                berth::berth_call(&self.berth, request).await
            }
            "settings_watch" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Args {
                    on_event: ChannelRef,
                    file: Option<ConfigFile>,
                }
                let Args { on_event, file } = parse(args)?;
                let sink = Box::new(self.sink(on_event.channel));
                settings::settings_watch(&self.settings, sink, file);
                Ok(Value::Null)
            }
            "connectors_watch" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Args {
                    on_event: ChannelRef,
                }
                let Args { on_event } = parse(args)?;
                let sink = Box::new(self.sink(on_event.channel));
                monitor::connectors_watch(&self.connectors, sink).await?;
                Ok(Value::Null)
            }
            "fs_watch" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Args {
                    path: String,
                    on_event: ChannelRef,
                }
                let Args { path, on_event } = parse(args)?;
                let sink = Box::new(self.sink(on_event.channel));
                Ok(json!(files::fs_watch(&self.files, path, sink)))
            }
            "agent_session_watch" => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Args {
                    since_days: Option<u32>,
                    on_event: ChannelRef,
                }
                let Args {
                    since_days,
                    on_event,
                } = parse(args)?;
                let sink = Box::new(self.sink(on_event.channel));
                agent_sessions::agent_session_watch(&self.sessions, since_days, sink);
                Ok(Value::Null)
            }
            "fs_unwatch" => {
                #[derive(Deserialize)]
                struct Args {
                    id: u32,
                }
                let Args { id } = parse(args)?;
                files::fs_unwatch(&self.files, id);
                Ok(Value::Null)
            }
            other => plain(&self.connectors, other, args)
                .await
                .unwrap_or_else(|| Err(refused(other))),
        }
    }
}
