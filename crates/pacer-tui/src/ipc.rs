//! Client-side IPC: connect to the daemon (auto-spawning it when absent) and
//! perform the version handshake.

use anyhow::{bail, Context, Result};
use pacer_core::codec::{read_frame, write_frame};
use pacer_core::transport::{self, Stream};
use pacer_core::{
    env, paths, AgentId, AgentKind, ClientRequest, EnterOutcome, ServerEvent, PROTOCOL_VERSION,
};
use std::time::Duration;

/// Request id for the one-shot CLIs: each opens a fresh connection, sends a
/// single request and waits for its reply, so there is never a second id.
const ONE_SHOT_REQ_ID: u64 = 1;
/// The daemon hung up mid-request — the message every one-shot client shows.
const CLOSED_BEFORE_REPLY: &str = "daemon closed the connection before replying";
/// How often the connect and shutdown waits re-check the daemon.
const POLL_STEP: Duration = Duration::from_millis(50);

pub struct Connection {
    pub stream: Stream,
    pub daemon_pid: u32,
}

/// Connect, auto-spawning `current_exe() daemon` when nothing is listening.
pub async fn connect_or_spawn() -> Result<Connection> {
    if let Ok(conn) = try_connect().await {
        return handshake(conn).await;
    }

    spawn_daemon()?;

    // Poll-connect while the daemon boots.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        match try_connect().await {
            Ok(conn) => return handshake(conn).await,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(POLL_STEP).await;
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "daemon did not come up on {} — check {}",
                        transport::endpoint_description(),
                        paths::daemon_log_path().display()
                    )
                })
            }
        }
    }
}

/// Reach the daemon over whatever the DAEMON SOCKET is on this platform,
/// clearing its authorization gate on the way (`pacer_core::transport`).
async fn try_connect() -> Result<Stream> {
    Ok(transport::connect().await?)
}

fn spawn_daemon() -> Result<()> {
    let exe = std::env::current_exe().context("resolve current_exe")?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    detach(&mut cmd);
    cmd.spawn().context("spawn pacer daemon")?;
    Ok(())
}

/// DAEMON SETSID: put the daemon in a new *session*, not just a new process
/// group. Besides outliving this client and skipping its terminal signals
/// (Ctrl+C etc.), the daemon must hold no controlling terminal. It shells out
/// to the user's interactive shell (CLI probes, login-shell agent wrap), and
/// an interactive zsh that can reach a tty via /dev/tty grabs its foreground
/// process group — SIGTTIN-stopping the TUI running on this terminal
/// mid-frame.
#[cfg(unix)]
fn detach(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            if libc_setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

// Avoid a libc dependency for one call (same pattern as pacer-core's geteuid).
#[cfg(unix)]
fn libc_setsid() -> i32 {
    extern "C" {
        fn setsid() -> i32;
    }
    unsafe { setsid() }
}

/// The Windows spelling of DAEMON SETSID.
///
/// `DETACHED_PROCESS` is the part that matters and the reason this is not
/// `CREATE_NO_WINDOW`: it gives the daemon *no console at all*, which is
/// what "no controlling terminal" means here. A daemon sharing this TUI's
/// console would let every child it starts write over the rendered frame and
/// would route the console's Ctrl+C to it as well. `CREATE_NEW_PROCESS_GROUP`
/// adds what `setsid` also gives: console control events aimed at this
/// client's group stop at the boundary.
#[cfg(windows)]
fn detach(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

async fn handshake(mut stream: Stream) -> Result<Connection> {
    write_frame(
        &mut stream,
        &ClientRequest::Hello {
            protocol_version: PROTOCOL_VERSION,
        },
    )
    .await?;
    match read_frame::<ServerEvent, _>(&mut stream).await? {
        Some(ServerEvent::HelloOk { daemon_pid, .. }) => Ok(Connection { stream, daemon_pid }),
        Some(ServerEvent::Incompatible {
            daemon_protocol_version,
        }) => bail!(version_skew_message(daemon_protocol_version)),
        other => bail!("unexpected handshake reply: {other:?}"),
    }
}

/// Explain a failed version handshake in terms of the fix.
///
/// Which side is stale decides the remedy, and getting it backwards costs a
/// debugging session: when the *daemon* is ahead, the `pacer` that just ran
/// is an older build than the one the daemon was launched from, and `pacer
/// kill` cannot help — the live instance immediately respawns its daemon
/// from its own binary (`spawn_daemon` above uses `current_exe`), so the
/// skew survives every restart. That is the common shape in a checkout,
/// where `make dev` runs `target/debug` while PATH still finds an older
/// `pacer` from the last `make install`.
fn version_skew_message(daemon_protocol_version: u32) -> String {
    let client = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let daemon = daemon_exe_path().unwrap_or_else(|| "unknown".into());
    let header = format!(
        "protocol mismatch: the daemon speaks v{daemon_protocol_version}, this client \
         v{PROTOCOL_VERSION}\n  this client: {client}\n  the daemon:  {daemon}\n"
    );
    if daemon_protocol_version > PROTOCOL_VERSION {
        format!(
            "{header}This client is the older build, so `pacer kill` will not fix it — the \
             running instance respawns its daemon from its own binary. Install the daemon's \
             build over this one instead (`make install` from that checkout)."
        )
    } else {
        format!(
            "{header}The daemon is the older build — run `pacer kill` and relaunch. That \
             stops every live session."
        )
    }
}

/// Best-effort path of the binary the running daemon was launched from, so
/// the mismatch message can name it. Read from the pidfile rather than the
/// handshake: `Incompatible` is what a *newer* daemon sends an older client,
/// so adding a field to it would only break decoding on the clients that
/// need this message most. The buildstamp beside the pidfile is no help
/// either — it is a content hash, not a path.
fn daemon_exe_path() -> Option<String> {
    let pid = std::fs::read_to_string(paths::pidfile_path()).ok()?;
    let pid = pid.trim();
    if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    exe_path_of_pid(pid)
}

#[cfg(unix)]
fn exe_path_of_pid(pid: &str) -> Option<String> {
    if let Ok(path) = std::fs::read_link(format!("/proc/{pid}/exe")) {
        return Some(path.display().to_string());
    }
    let out = std::process::Command::new("ps")
        .args(["-p", pid, "-o", "comm="])
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then_some(path)
}

/// Windows has neither `/proc` nor `ps`; ask the kernel directly. This used
/// to shell out to `wmic`, which Windows 11 24H2 no longer ships.
#[cfg(windows)]
fn exe_path_of_pid(pid: &str) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let pid: u32 = pid.parse().ok()?;
    // SAFETY: OpenProcess returns null on failure; the handle is closed on
    // every path out, and the buffer outlives the query that fills it.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 || len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Channel-based IPC handle for the TUI event loop: outbound requests go
/// through `tx`; inbound events arrive on `rx`. Reader/writer tasks own the
/// socket halves.
pub struct IpcChannels {
    pub tx: tokio::sync::mpsc::Sender<ClientRequest>,
    pub rx: tokio::sync::mpsc::Receiver<ServerEvent>,
}

pub fn split_connection(conn: Connection) -> IpcChannels {
    let (read_half, mut write_half) = conn.stream.into_split();
    let (event_tx, event_rx) = tokio::sync::mpsc::channel::<ServerEvent>(1024);
    let (req_tx, mut req_rx) = tokio::sync::mpsc::channel::<ClientRequest>(256);

    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(read_half);
        while let Ok(Some(ev)) = read_frame::<ServerEvent, _>(&mut reader).await {
            if event_tx.send(ev).await.is_err() {
                break;
            }
        }
        // Dropping event_tx closes the channel, signalling disconnect.
    });

    tokio::spawn(async move {
        while let Some(req) = req_rx.recv().await {
            if write_frame(&mut write_half, &req).await.is_err() {
                break;
            }
        }
    });

    IpcChannels {
        tx: req_tx,
        rx: event_rx,
    }
}

/// The agent id a one-shot CLI runs as, from the raw `NEBULA_AGENT_ID`
/// value. Unset and empty are the same miss, and the error names the
/// `pacer <verb>` that needs it. Pure so the message is testable.
fn agent_id_from(value: Option<String>, verb: &str) -> Result<String> {
    value.filter(|v| !v.is_empty()).with_context(|| {
        format!(
            "{} is not set — `pacer {verb}` only works from inside a \
             pacer agent session",
            env::AGENT_ID
        )
    })
}

/// [`agent_id_from`] over the live environment.
fn current_agent_id(verb: &str) -> Result<String> {
    agent_id_from(env::non_empty(env::AGENT_ID), verb)
}

/// What the daemon said back to a one-shot request.
enum Reply {
    Ack,
    /// The daemon's own message for a request it declined.
    Error(String),
}

/// Read events until the daemon answers `req_id` — an Ack or an Error —
/// skipping anything else it broadcasts meanwhile. Only the hang-up is an
/// `Err`; the daemon's refusal comes back as a [`Reply::Error`] so each
/// caller can decide whether that is a failure.
async fn await_reply(conn: &mut Connection, req_id: u64) -> Result<Reply> {
    loop {
        match read_frame::<ServerEvent, _>(&mut conn.stream).await? {
            Some(ServerEvent::Ack { req_id: r, .. }) if r == req_id => return Ok(Reply::Ack),
            Some(ServerEvent::Error {
                req_id: Some(r),
                message,
            }) if r == req_id => return Ok(Reply::Error(message)),
            Some(_) => continue,
            None => bail!("{CLOSED_BEFORE_REPLY}"),
        }
    }
}

/// [`await_reply`] for the callers where the daemon declining *is* the
/// failure: its message becomes the error.
async fn await_ack(conn: &mut Connection, req_id: u64) -> Result<()> {
    match await_reply(conn, req_id).await? {
        Reply::Ack => Ok(()),
        Reply::Error(message) => bail!("{message}"),
    }
}

/// How `pacer rename` treats a session that already carries a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameMode {
    /// Title only an untitled session; the daemon declines otherwise.
    Auto,
    /// Overwrite whatever title the session has (`--force`).
    Force,
}

/// One-shot client for `pacer rename`, run from inside an agent session's
/// CLI: resolve the agent from NEBULA_AGENT_ID and ask the daemon to title
/// it. Never spawns a daemon — no daemon means no session worth titling.
///
/// Daemon-reported outcomes (renamed, or "already titled" on the non-force
/// path) both print and exit 0: for the model running this, a declined
/// auto-title is a settled answer, not a failure to retry.
pub async fn rename_current_agent(title: &str, mode: RenameMode) -> Result<()> {
    let agent_id = current_agent_id("rename")?;
    let Ok(stream) = try_connect().await else {
        bail!("no pacer daemon is running — title unchanged");
    };
    let mut conn = handshake(stream).await?;
    let req_id = ONE_SHOT_REQ_ID;
    let id = AgentId(agent_id);
    let name = title.to_string();
    let request = match mode {
        RenameMode::Force => ClientRequest::RenameAgent { req_id, id, name },
        RenameMode::Auto => ClientRequest::AutoRenameAgent { req_id, id, name },
    };
    write_frame(&mut conn.stream, &request).await?;
    match await_reply(&mut conn, req_id).await? {
        Reply::Ack => println!("session renamed to \"{title}\""),
        Reply::Error(message) => println!("pacer: {message}"),
    }
    Ok(())
}

/// CLI: `pacer spawn "<task>" [--kind <claude|codex|cursor>]` from inside
/// an agent session — ask the daemon to start a new agent beside this one,
/// in the same worktree, opening on `task` as its first prompt. The caller
/// is untouched: no relocation, no turn-end wait. Never spawns a daemon: no
/// daemon means no session to sit beside.
///
/// What this prints is read by the model that ran it, so it says what
/// happened and that this session carries on. A daemon-side refusal (a
/// blank task, a missing CLI) is a nonzero exit the model reports.
pub async fn spawn_sibling_for_current_agent(task: &str, kind: Option<AgentKind>) -> Result<()> {
    let agent_id = current_agent_id("spawn")?;
    let task = task.trim();
    if task.is_empty() {
        bail!("the task is empty — `pacer spawn \"<task>\"` needs the work the new session starts on");
    }
    let Ok(stream) = try_connect().await else {
        bail!("no pacer daemon is running — no session started");
    };
    let mut conn = handshake(stream).await?;
    let req_id = ONE_SHOT_REQ_ID;
    write_frame(
        &mut conn.stream,
        &ClientRequest::SpawnSiblingAgent {
            req_id,
            id: AgentId(agent_id),
            kind,
            starting_prompt: task.to_string(),
        },
    )
    .await?;
    await_ack(&mut conn, req_id).await?;
    let harness = kind.map(|k| format!("{} ", k.as_str())).unwrap_or_default();
    println!(
        "started a new {harness}session in this worktree; it is working on that task now and \
         shows in the sessions list. This session is unaffected — carry on."
    );
    Ok(())
}

/// CLI: `pacer worktree [name] [--base <ref>]` from inside an agent
/// session — take (or create) the named worktree of this session's project
/// and have the daemon move the session into it. A blank name is invented
/// the way the TUI's new-worktree prompt invents one, and spaces slugify to
/// hyphens the same way. Never spawns a daemon: no daemon means no session
/// to move.
///
/// What this prints is read by the model that ran it, so the relocating
/// case spells out what happens next: the daemon kills and resumes this
/// very process once the turn ends, and the answer has to be finished by
/// then. A daemon-side failure is a nonzero exit — the model reports it and
/// stays put.
pub async fn enter_worktree_for_current_agent(name: &str, base: Option<String>) -> Result<()> {
    let agent_id = current_agent_id("worktree")?;
    let branch = match crate::branch_name::slugify(name) {
        slug if slug.is_empty() => crate::branch_name::random_name(&[]),
        slug => slug,
    };
    let Ok(stream) = try_connect().await else {
        bail!("no pacer daemon is running — nothing to move");
    };
    let mut conn = handshake(stream).await?;
    let req_id = ONE_SHOT_REQ_ID;
    write_frame(
        &mut conn.stream,
        &ClientRequest::EnterWorktree {
            req_id,
            id: AgentId(agent_id),
            branch,
            base,
        },
    )
    .await?;
    loop {
        match read_frame::<ServerEvent, _>(&mut conn.stream).await? {
            Some(ServerEvent::WorktreeEntered {
                req_id: r,
                worktree,
                outcome,
            }) if r == req_id => {
                println!(
                    "worktree \"{}\" is ready at {}",
                    worktree.branch,
                    worktree.path.display()
                );
                match outcome {
                    EnterOutcome::AlreadyThere => {
                        println!("this session already runs inside it — nothing to move.");
                    }
                    EnterOutcome::Relocating => {
                        println!(
                            "this session is now associated with it; pacer will relocate the \
                             session into it the moment this turn ends."
                        );
                        println!(
                            "Finish now: tell the user in one line that the session is moving \
                             into the worktree, and make no further tool calls or edits — you \
                             will be resumed inside the worktree with a prompt to continue."
                        );
                    }
                    EnterOutcome::NextLaunch => {
                        println!(
                            "this session is now associated with it and runs there from its \
                             next launch."
                        );
                    }
                }
                return Ok(());
            }
            Some(ServerEvent::Error {
                req_id: Some(r),
                message,
            }) if r == req_id => bail!("{message}"),
            Some(_) => continue,
            None => bail!("{CLOSED_BEFORE_REPLY}"),
        }
    }
}

/// One-shot client for `pacer add <dir>` (and bare `pacer <dir>`): resolve
/// the path locally — the daemon's cwd is not ours, so relative paths must be
/// absolutized here — and ask the daemon to register it as a project. The
/// daemon owns the rest: normalizing to the repo toplevel, naming the project
/// after the directory, rejecting non-repos and duplicates. Spawns a daemon
/// when none is running, same as launching the TUI would.
pub async fn add_project(path: &str) -> Result<()> {
    let expanded = match (path.strip_prefix("~/"), env::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => std::path::PathBuf::from(path),
    };
    // Canonicalizing is the existence check, but the *stored* path has to be
    // one every consumer can take: this one is handed back to git, so a
    // verbatim `\\?\D:\…` would be a project pacer could never run git in.
    std::fs::canonicalize(&expanded)
        .with_context(|| format!("{} does not exist", expanded.display()))?;
    let dir = paths::canonical_or_raw(&expanded);
    if !dir.is_dir() {
        bail!("{} is not a directory", dir.display());
    }
    let mut conn = connect_or_spawn().await?;
    let req_id = ONE_SHOT_REQ_ID;
    write_frame(
        &mut conn.stream,
        &ClientRequest::AddProject {
            req_id,
            path: dir.clone(),
            name: None,
            create_missing: false,
        },
    )
    .await?;
    await_ack(&mut conn, req_id).await?;
    println!("added project {}", dir.display());
    Ok(())
}

/// One `pacer workspace <op>` invocation, resolved and executed against the
/// daemon (spawned when absent, same as `pacer add`).
#[derive(Debug, Clone)]
pub enum WorkspaceOp {
    Add { name: String },
    Open { name: String },
    List,
    Delete { name: String },
    Rename { name: String, new_name: String },
}

/// One-shot client for `pacer workspace …`. Name→id resolution runs off a
/// snapshot (Subscribe's first reply), so the daemon's RPC surface stays
/// id-based for the TUI's picker.
pub async fn run_workspace_op(op: WorkspaceOp) -> Result<()> {
    use pacer_core::{Workspace, WorkspaceId};
    let mut conn = connect_or_spawn().await?;
    write_frame(&mut conn.stream, &ClientRequest::Subscribe).await?;
    let (workspaces, active, projects) = loop {
        match read_frame::<ServerEvent, _>(&mut conn.stream).await? {
            Some(ServerEvent::Snapshot {
                workspaces,
                active_workspace,
                projects,
                ..
            }) => break (workspaces, active_workspace, projects),
            Some(_) => continue,
            None => bail!("daemon closed the connection before sending a snapshot"),
        }
    };
    let resolve = |name: &str| -> Result<WorkspaceId> {
        workspaces
            .iter()
            .find(|w: &&Workspace| w.name == name)
            .map(|w| w.id.clone())
            .with_context(|| {
                let names: Vec<&str> = workspaces.iter().map(|w| w.name.as_str()).collect();
                format!(
                    "no workspace named '{name}' (available: {})",
                    names.join(", ")
                )
            })
    };
    let req_id = ONE_SHOT_REQ_ID;
    let (request, done): (ClientRequest, String) = match op {
        WorkspaceOp::List => {
            for w in &workspaces {
                let marker = if w.id == active { "*" } else { " " };
                let count = projects.iter().filter(|p| p.workspace_id == w.id).count();
                println!(
                    "{marker} {}  ({count} project{})",
                    w.name,
                    if count == 1 { "" } else { "s" }
                );
            }
            return Ok(());
        }
        WorkspaceOp::Add { name } => (
            ClientRequest::AddWorkspace {
                req_id,
                name: name.clone(),
            },
            format!("workspace '{name}' added — open it with `pacer workspace open {name}`"),
        ),
        WorkspaceOp::Open { name } => (
            ClientRequest::OpenWorkspace {
                req_id,
                id: resolve(&name)?,
            },
            // Running instances keep the workspace their user put them on;
            // this sets where the next one starts. `pacer --workspace
            // <name>` is the way to aim one instance without moving this.
            format!("workspace '{name}' will open in new pacer instances"),
        ),
        WorkspaceOp::Delete { name } => (
            ClientRequest::RemoveWorkspace {
                req_id,
                id: resolve(&name)?,
            },
            format!("workspace '{name}' deleted"),
        ),
        WorkspaceOp::Rename { name, new_name } => (
            ClientRequest::RenameWorkspace {
                req_id,
                id: resolve(&name)?,
                name: new_name.clone(),
            },
            format!("workspace '{name}' renamed to '{new_name}'"),
        ),
    };
    write_frame(&mut conn.stream, &request).await?;
    await_ack(&mut conn, req_id).await?;
    println!("{done}");
    Ok(())
}

/// Ask a running daemon to shut down. Ok(false) when none is running.
///
/// A daemon on a different protocol version closes the socket right after
/// the handshake, so `Shutdown` can never reach it — exactly the situation
/// `pacer kill` exists to fix. Fall back to terminating the pid in the
/// pidfile, guarded by the daemon's own lock so a stale pid is never hit.
pub async fn kill_daemon() -> Result<bool> {
    if let Ok(stream) = try_connect().await {
        if let Ok(mut conn) = handshake(stream).await {
            write_frame(&mut conn.stream, &ClientRequest::Shutdown).await?;
            wait_for_daemon_exit().await;
            return Ok(true);
        }
        return kill_by_pidfile().await;
    }
    // Nothing listening — but a wedged or mid-boot daemon may still hold the
    // pidfile lock; fall through to the same check.
    kill_by_pidfile().await
}

/// Outcome of `shutdown_if_idle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleShutdown {
    /// Nothing listening on the socket.
    NoDaemon,
    /// The daemon held no live PTYs and was shut down cleanly.
    ShutDown,
    /// Live sessions exist; the daemon was left running.
    SessionsLive { count: usize },
    /// A daemon is listening but its protocol version differs, so its
    /// session state can't be inspected.
    Skewed,
}

/// Shut the daemon down only when it holds no live PTYs — the post-upgrade
/// handoff. An idle daemon can die safely (the next client launch spawns one
/// from the new binary on disk); live sessions would be killed with it, so
/// their daemon is left alone and the restart stays the user's call.
pub async fn shutdown_if_idle() -> Result<IdleShutdown> {
    let Ok(stream) = try_connect().await else {
        return Ok(IdleShutdown::NoDaemon);
    };
    let Ok(mut conn) = handshake(stream).await else {
        return Ok(IdleShutdown::Skewed);
    };
    write_frame(&mut conn.stream, &ClientRequest::Subscribe).await?;
    loop {
        match read_frame::<ServerEvent, _>(&mut conn.stream).await? {
            Some(ServerEvent::Snapshot {
                agents, terminals, ..
            }) => {
                let live = agents.iter().filter(|a| a.alive).count()
                    + terminals.iter().filter(|t| t.alive).count();
                if live > 0 {
                    return Ok(IdleShutdown::SessionsLive { count: live });
                }
                write_frame(&mut conn.stream, &ClientRequest::Shutdown).await?;
                wait_for_daemon_exit().await;
                return Ok(IdleShutdown::ShutDown);
            }
            Some(_) => continue,
            None => bail!("daemon closed the connection before sending a snapshot"),
        }
    }
}

/// Terminate the daemon recorded in the pidfile, guarded by the pidfile lock
/// so a stale pid is never signalled. Ok(false) when no daemon is alive.
async fn kill_by_pidfile() -> Result<bool> {
    let path = paths::pidfile_path();
    if !daemon_holds_pidfile_lock(&path) {
        return Ok(false);
    }
    let pid: i32 = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .filter(|pid| *pid > 0)
        .context("daemon is running but its pidfile is unreadable — kill it manually")?;
    if !request_termination(pid) {
        bail!("failed to signal daemon pid {pid} — kill it manually");
    }
    wait_for_daemon_exit().await;
    Ok(true)
}

/// Liveness = lock possession (mirrors the daemon's PidfileLock): if we can
/// take the lock ourselves, nobody holds it. Released on drop.
fn daemon_holds_pidfile_lock(path: &std::path::Path) -> bool {
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    else {
        return false;
    };
    !try_lock_exclusive(&file)
}

/// Poll until the daemon releases its pidfile lock, so a relaunch right after
/// `pacer kill` can't race the old daemon's teardown.
async fn wait_for_daemon_exit() {
    let path = paths::pidfile_path();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while daemon_holds_pidfile_lock(&path) && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(POLL_STEP).await;
    }
}

// Tiny extern shims, same dep-light idiom as pacer_core::paths. They must
// stay equivalent to the daemon's own `lifecycle` versions: this side asks
// the same question of the same file from another process.
#[cfg(unix)]
fn try_lock_exclusive(file: &std::fs::File) -> bool {
    use std::os::fd::AsRawFd;
    extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) == 0 }
}

/// The byte the PIDFILE LOCK is taken on. Must equal
/// `pacer_daemon::lifecycle::LOCK_OFFSET`; `pacer-tui` does not depend on
/// `pacer-daemon`, so the agreement is asserted by the Windows smoke grid
/// rather than by the compiler.
#[cfg(windows)]
const PIDFILE_LOCK_OFFSET: u32 = 0x4000_0000;

/// See `pacer_daemon::lifecycle`: one byte at the same far offset it uses,
/// non-blocking. The offset has to match or the two never contend — and it
/// has to be past the content, because a Windows lock is mandatory and would
/// otherwise make the pidfile unreadable to `kill_by_pidfile` below.
#[cfg(windows)]
fn try_lock_exclusive(file: &std::fs::File) -> bool {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: *mut core::ffi::c_void,
    }
    extern "system" {
        fn LockFileEx(
            handle: *mut core::ffi::c_void,
            flags: u32,
            reserved: u32,
            bytes_low: u32,
            bytes_high: u32,
            overlapped: *mut Overlapped,
        ) -> i32;
    }
    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x1;
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x2;
    let mut overlapped = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: PIDFILE_LOCK_OFFSET,
        offset_high: 0,
        event: std::ptr::null_mut(),
    };
    unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        ) != 0
    }
}

/// Ask the daemon at `pid` to stop, the hard way. True when the request was
/// delivered.
#[cfg(unix)]
fn request_termination(pid: i32) -> bool {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const SIGTERM: i32 = 15;
    unsafe { kill(pid, SIGTERM) == 0 }
}

/// Windows has no SIGTERM, and a DETACHED_PROCESS daemon has its own console,
/// so no console control event of ours reaches it either. What is left is
/// `TerminateProcess` — abrupt, so it skips the daemon's PTY SESSION
/// teardown; the Job Objects in `pty::kill` are what keeps that from
/// stranding agent CLIs (they die when the daemon's handles close).
///
/// This is the fallback path only: an ordinary NEBULA KILL reaches the daemon
/// over the DAEMON SOCKET and gets the clean shutdown.
#[cfg(windows)]
fn request_termination(pid: i32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid as u32);
        if handle.is_null() {
            return false;
        }
        let ok = TerminateProcess(handle, 1) != 0;
        CloseHandle(handle);
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unset and empty are the same miss, and the error has to name the
    // command the model just ran so it knows why it can't work here.
    #[test]
    fn agent_id_requires_a_non_empty_value_and_names_the_verb() {
        let err = agent_id_from(None, "rename").unwrap_err().to_string();
        assert!(err.contains("`pacer rename`"), "{err}");
        assert!(err.contains(env::AGENT_ID), "{err}");
        let err = agent_id_from(Some(String::new()), "worktree")
            .unwrap_err()
            .to_string();
        assert!(err.contains("`pacer worktree`"), "{err}");
        assert_eq!(agent_id_from(Some("a1".into()), "rename").unwrap(), "a1");
    }

    // The whole point of the message: `pacer kill` is the fix for exactly
    // one of the two skews, and recommending it for the other sends the user
    // in a circle (kill the daemon, the live TUI respawns the same one).
    #[test]
    fn skew_message_blames_the_older_side() {
        let daemon_ahead = version_skew_message(PROTOCOL_VERSION + 2);
        assert!(daemon_ahead.contains("This client is the older build"));
        assert!(
            !daemon_ahead.contains("run `pacer kill` and relaunch"),
            "must not send the user to kill a daemon that is not the stale side: {daemon_ahead}"
        );
        assert!(daemon_ahead.contains("make install"));

        let daemon_behind = version_skew_message(PROTOCOL_VERSION - 1);
        assert!(daemon_behind.contains("The daemon is the older build"));
        assert!(daemon_behind.contains("run `pacer kill` and relaunch"));
    }

    #[test]
    fn skew_message_names_both_binaries() {
        let msg = version_skew_message(PROTOCOL_VERSION + 1);
        assert!(msg.contains("this client: "), "{msg}");
        assert!(msg.contains("the daemon:  "), "{msg}");
        // current_exe resolves under a test binary, so this half is never
        // the "unknown" fallback.
        assert!(msg.contains(&format!("v{PROTOCOL_VERSION}")), "{msg}");
    }
}
