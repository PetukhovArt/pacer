use std::collections::HashMap;
use std::io;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use pacer_core::codec::{read_frame, write_frame};
use pacer_core::transport::{self, Stream};
use pacer_core::{
    Agent, AgentId, AgentStatus, ClientRequest, Entity, EntityId, Project, ServerEvent, SessionRef,
    TerminalTab, Workspace, WorkspaceId, Worktree, PROTOCOL_VERSION,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use tokio::io::{BufReader, BufWriter, ReadHalf, WriteHalf};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::AbortHandle;

const OFFLINE_RETRY: Duration = Duration::from_secs(1);
const INCOMPATIBLE_RETRY: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connecting,
    Connected,
    Offline,
    Incompatible,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WatchEvent {
    Connection {
        state: ConnectionState,
        message: Option<String>,
    },
    Snapshot {
        workspaces: Vec<Workspace>,
        active_workspace: WorkspaceId,
        projects: Vec<Project>,
        worktrees: Vec<Worktree>,
        agents: Vec<Agent>,
        terminals: Vec<TerminalTab>,
    },
    Upserted {
        entity: Entity,
    },
    Removed {
        id: EntityId,
    },
    Status {
        agent: AgentId,
        status: AgentStatus,
        changed_at: i64,
        unseen: bool,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    Reset { data: String },
    Output { data: String },
    Exited { exit_code: Option<i32> },
}

pub struct Pacer(UnboundedSender<Msg>);

impl Pacer {
    pub fn spawn() -> Self {
        let (tx, rx) = unbounded_channel();
        tauri::async_runtime::spawn(Actor::new(tx.clone()).run(rx));
        Self(tx)
    }

    fn send(&self, command: Command) {
        let _ = self.0.send(Msg::Command(command));
    }
}

#[tauri::command]
pub fn pacer_watch(pacer: State<'_, Pacer>, on_event: Channel<WatchEvent>) {
    pacer.send(Command::Watch(on_event));
}

#[tauri::command]
pub fn pacer_attach(
    pacer: State<'_, Pacer>,
    session: SessionRef,
    cols: u16,
    rows: u16,
    on_event: Channel<SessionEvent>,
) {
    pacer.send(Command::Attach {
        session,
        cols,
        rows,
        channel: on_event,
    });
}

#[tauri::command]
pub fn pacer_input(pacer: State<'_, Pacer>, session: SessionRef, data: String) {
    pacer.send(Command::Input {
        session,
        data: data.into_bytes(),
    });
}

#[tauri::command]
pub fn pacer_resize(pacer: State<'_, Pacer>, session: SessionRef, cols: u16, rows: u16) {
    pacer.send(Command::Resize {
        session,
        cols,
        rows,
    });
}

#[tauri::command]
pub fn pacer_detach(pacer: State<'_, Pacer>, session: SessionRef) {
    pacer.send(Command::Detach { session });
}

#[tauri::command]
pub fn pacer_mark_seen(pacer: State<'_, Pacer>, agent: AgentId) {
    pacer.send(Command::MarkSeen { agent });
}

enum Msg {
    Command(Command),
    Link { epoch: u64, event: LinkEvent },
}

enum Command {
    Watch(Channel<WatchEvent>),
    Attach {
        session: SessionRef,
        cols: u16,
        rows: u16,
        channel: Channel<SessionEvent>,
    },
    Input {
        session: SessionRef,
        data: Vec<u8>,
    },
    Resize {
        session: SessionRef,
        cols: u16,
        rows: u16,
    },
    Detach {
        session: SessionRef,
    },
    MarkSeen {
        agent: AgentId,
    },
}

enum LinkEvent {
    Up(Stream),
    Failed(Failure),
    RetryDue,
    Frame(ServerEvent),
    Down,
}

enum Failure {
    Offline(String),
    Incompatible(u32),
}

impl Failure {
    fn state(&self) -> ConnectionState {
        match self {
            Self::Offline(_) => ConnectionState::Offline,
            Self::Incompatible(_) => ConnectionState::Incompatible,
        }
    }

    fn retry_after(&self) -> Duration {
        match self {
            Self::Offline(_) => OFFLINE_RETRY,
            Self::Incompatible(_) => INCOMPATIBLE_RETRY,
        }
    }

    fn message(self) -> String {
        match self {
            Self::Offline(message) => message,
            Self::Incompatible(daemon) => format!(
                "pacer daemon speaks protocol v{daemon}, Veyra speaks protocol v{PROTOCOL_VERSION}"
            ),
        }
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self::Offline(error.to_string())
    }
}

struct Attachment {
    channel: Channel<SessionEvent>,
    next_seq: Option<u64>,
    replaying: bool,
    cols: u16,
    rows: u16,
}

impl Attachment {
    fn attach_request(&mut self, session: SessionRef) -> ClientRequest {
        self.replaying = true;
        ClientRequest::Attach {
            session,
            from_seq: self.next_seq,
            cols: self.cols,
            rows: self.rows,
        }
    }

    fn scrollback(&mut self, base_seq: u64, data: Vec<u8>) {
        let continues = self.next_seq == Some(base_seq);
        self.next_seq = Some(base_seq + data.len() as u64);
        self.replaying = false;
        let data = STANDARD.encode(data);
        let _ = self.channel.send(if continues {
            SessionEvent::Output { data }
        } else {
            SessionEvent::Reset { data }
        });
    }

    fn output(&mut self, seq: u64, data: Vec<u8>) {
        if self.replaying {
            return;
        }
        let end = seq + data.len() as u64;
        let start = self.next_seq.map_or(seq, |next| next.max(seq));
        if end <= start {
            return;
        }
        self.next_seq = Some(end);
        let data = STANDARD.encode(&data[(start - seq) as usize..]);
        let _ = self.channel.send(SessionEvent::Output { data });
    }

    fn exited(&self, exit_code: Option<i32>) {
        let _ = self.channel.send(SessionEvent::Exited { exit_code });
    }
}

struct Actor {
    inbox: UnboundedSender<Msg>,
    watcher: Option<Channel<WatchEvent>>,
    shown: Option<(ConnectionState, Option<String>)>,
    attachments: HashMap<SessionRef, Attachment>,
    epoch: u64,
    tasks: Vec<AbortHandle>,
    writer: Option<UnboundedSender<ClientRequest>>,
}

impl Actor {
    fn new(inbox: UnboundedSender<Msg>) -> Self {
        Self {
            inbox,
            watcher: None,
            shown: None,
            attachments: HashMap::new(),
            epoch: 0,
            tasks: Vec::new(),
            writer: None,
        }
    }

    async fn run(mut self, mut rx: UnboundedReceiver<Msg>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                Msg::Command(command) => self.command(command),
                Msg::Link { epoch, event } if epoch == self.epoch => self.link(event),
                Msg::Link { .. } => {}
            }
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Watch(channel) => {
                self.watcher = Some(channel);
                self.shown = None;
                self.attachments.clear();
                self.show(ConnectionState::Connecting, None);
                self.connect();
            }
            Command::Attach {
                session,
                cols,
                rows,
                channel,
            } => {
                let mut attachment = Attachment {
                    channel,
                    next_seq: None,
                    replaying: true,
                    cols,
                    rows,
                };
                self.write(attachment.attach_request(session.clone()));
                self.attachments.insert(session, attachment);
            }
            Command::Input { session, data } => self.write(ClientRequest::Input { session, data }),
            Command::Resize {
                session,
                cols,
                rows,
            } => {
                if let Some(attachment) = self.attachments.get_mut(&session) {
                    attachment.cols = cols;
                    attachment.rows = rows;
                }
                self.write(ClientRequest::Resize {
                    session,
                    cols,
                    rows,
                });
            }
            Command::Detach { session } => {
                self.attachments.remove(&session);
                self.write(ClientRequest::Detach { session });
            }
            Command::MarkSeen { agent } => self.write(ClientRequest::MarkAgentSeen { id: agent }),
        }
    }

    fn link(&mut self, event: LinkEvent) {
        match event {
            LinkEvent::Up(stream) => self.up(stream),
            LinkEvent::Failed(failure) => {
                self.tasks.clear();
                let retry_after = failure.retry_after();
                self.show(failure.state(), Some(failure.message()));
                let inbox = self.inbox.clone();
                let epoch = self.epoch;
                self.spawn(async move {
                    tokio::time::sleep(retry_after).await;
                    let _ = inbox.send(Msg::Link {
                        epoch,
                        event: LinkEvent::RetryDue,
                    });
                });
            }
            LinkEvent::RetryDue => self.connect(),
            LinkEvent::Frame(event) => self.frame(event),
            LinkEvent::Down => {
                self.show(ConnectionState::Connecting, None);
                self.connect();
            }
        }
    }

    fn connect(&mut self) {
        self.drop_link();
        self.epoch += 1;
        let inbox = self.inbox.clone();
        let epoch = self.epoch;
        self.spawn(async move {
            let event = match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake()).await {
                Ok(Ok(stream)) => LinkEvent::Up(stream),
                Ok(Err(failure)) => LinkEvent::Failed(failure),
                Err(_) => {
                    LinkEvent::Failed(Failure::Offline("pacer daemon handshake timed out".into()))
                }
            };
            let _ = inbox.send(Msg::Link { epoch, event });
        });
    }

    fn up(&mut self, stream: Stream) {
        self.tasks.clear();
        let (reader, writer) = tokio::io::split(stream);
        let (writer_tx, writer_rx) = unbounded_channel();
        self.spawn(read_loop(
            BufReader::new(reader),
            self.epoch,
            self.inbox.clone(),
        ));
        self.spawn(write_loop(
            BufWriter::new(writer),
            writer_rx,
            self.epoch,
            self.inbox.clone(),
        ));
        let _ = writer_tx.send(ClientRequest::Subscribe);
        for (session, attachment) in &mut self.attachments {
            let _ = writer_tx.send(attachment.attach_request(session.clone()));
        }
        self.writer = Some(writer_tx);
        self.show(ConnectionState::Connected, None);
    }

    fn frame(&mut self, event: ServerEvent) {
        let watch = match event {
            ServerEvent::Snapshot {
                workspaces,
                active_workspace,
                projects,
                worktrees,
                agents,
                terminals,
                ..
            } => WatchEvent::Snapshot {
                workspaces,
                active_workspace,
                projects,
                worktrees,
                agents,
                terminals,
            },
            ServerEvent::EntityUpserted { entity } => WatchEvent::Upserted { entity },
            ServerEvent::EntityRemoved { id } => WatchEvent::Removed { id },
            ServerEvent::StatusChanged {
                agent,
                status,
                changed_at,
                unseen,
            } => WatchEvent::Status {
                agent,
                status,
                changed_at,
                unseen,
            },
            ServerEvent::Error { message, .. } => WatchEvent::Error { message },
            ServerEvent::Scrollback {
                session,
                base_seq,
                data,
            } => {
                if let Some(attachment) = self.attachments.get_mut(&session) {
                    attachment.scrollback(base_seq, data);
                }
                return;
            }
            ServerEvent::Output { session, seq, data } => {
                if let Some(attachment) = self.attachments.get_mut(&session) {
                    attachment.output(seq, data);
                }
                return;
            }
            ServerEvent::SessionExited { session, exit_code } => {
                if let Some(attachment) = self.attachments.get(&session) {
                    attachment.exited(exit_code);
                }
                return;
            }
            _ => return,
        };
        self.emit(watch);
    }

    fn write(&self, request: ClientRequest) {
        if let Some(writer) = &self.writer {
            let _ = writer.send(request);
        }
    }

    fn show(&mut self, state: ConnectionState, message: Option<String>) {
        let shown = Some((state, message.clone()));
        if self.shown == shown {
            return;
        }
        self.shown = shown;
        self.emit(WatchEvent::Connection { state, message });
    }

    fn emit(&self, event: WatchEvent) {
        if let Some(watcher) = &self.watcher {
            let _ = watcher.send(event);
        }
    }

    fn spawn(&mut self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        self.tasks.push(tokio::spawn(task).abort_handle());
    }

    fn drop_link(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
        self.writer = None;
    }
}

async fn handshake() -> Result<Stream, Failure> {
    let mut stream = transport::connect().await?;
    write_frame(
        &mut stream,
        &ClientRequest::Hello {
            protocol_version: PROTOCOL_VERSION,
        },
    )
    .await?;
    match read_frame::<ServerEvent, _>(&mut stream).await? {
        Some(ServerEvent::HelloOk { .. }) => Ok(stream),
        Some(ServerEvent::Incompatible {
            daemon_protocol_version,
        }) => Err(Failure::Incompatible(daemon_protocol_version)),
        Some(other) => Err(Failure::Offline(format!(
            "unexpected pacer handshake reply: {other:?}"
        ))),
        None => Err(Failure::Offline(
            "pacer daemon closed the connection during the handshake".into(),
        )),
    }
}

async fn read_loop(
    mut reader: BufReader<ReadHalf<Stream>>,
    epoch: u64,
    inbox: UnboundedSender<Msg>,
) {
    while let Ok(Some(event)) = read_frame::<ServerEvent, _>(&mut reader).await {
        let frame = Msg::Link {
            epoch,
            event: LinkEvent::Frame(event),
        };
        if inbox.send(frame).is_err() {
            return;
        }
    }
    let _ = inbox.send(Msg::Link {
        epoch,
        event: LinkEvent::Down,
    });
}

async fn write_loop(
    mut writer: BufWriter<WriteHalf<Stream>>,
    mut requests: UnboundedReceiver<ClientRequest>,
    epoch: u64,
    inbox: UnboundedSender<Msg>,
) {
    while let Some(request) = requests.recv().await {
        if write_frame(&mut writer, &request).await.is_err() {
            let _ = inbox.send(Msg::Link {
                epoch,
                event: LinkEvent::Down,
            });
            return;
        }
    }
}
