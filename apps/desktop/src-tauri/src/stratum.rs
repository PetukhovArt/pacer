use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

use crate::child;
use crate::connectors::types::Kind;
use crate::connectors::Connectors;
use crate::settings::berth_dir;

const CONFIG: &str = "stratum.config.jsonc";
const READY_MARK: &str = "stratum-visualizer ready at ";
const MAX_SERVERS: usize = 3;
const READY_TIMEOUT: Duration = Duration::from_secs(120);
const GIT_TIMEOUT: Duration = Duration::from_secs(180);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const RELEASE_GRACE: Duration = Duration::from_secs(20);
const LOG_KEEP: usize = 80;
const NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Progress {
    phase: String,
    lines: Vec<String>,
}

type Log = Arc<Mutex<Progress>>;

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn phase(log: &Log, name: &str) {
    locked(log).phase = name.to_owned();
}

fn push(log: &Log, line: impl Into<String>) {
    let mut guard = locked(log);
    guard.lines.push(line.into());
    let extra = guard.lines.len().saturating_sub(LOG_KEEP);
    guard.lines.drain(..extra);
}

struct Server {
    key: String,
    repo: PathBuf,
    dir: PathBuf,
    pid: u32,
    url: String,
    head: String,
    base: String,
    child: tokio::sync::Mutex<Child>,
    leases: Mutex<usize>,
    used: Mutex<Instant>,
}

#[derive(Default)]
struct Manager {
    servers: Mutex<HashMap<String, Arc<Server>>>,
    starting: tokio::sync::Mutex<()>,
    progress: Mutex<HashMap<String, Log>>,
}

fn manager() -> &'static Manager {
    static MANAGER: OnceLock<Manager> = OnceLock::new();
    MANAGER.get_or_init(Manager::default)
}

#[derive(Debug, Serialize)]
pub struct Probe {
    config: bool,
    linter: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Visualizer {
    key: String,
    url: String,
    head: String,
    base: String,
}

fn clean(path: &str) -> PathBuf {
    PathBuf::from(path.strip_prefix(r"\\?\").unwrap_or(path))
}

fn resolve_linter(repo: &Path) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["stratum-lint.cmd", "stratum-lint.exe"]
    } else {
        &["stratum-lint"]
    };
    let pinned = repo.join("node_modules").join(".bin");
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::iter::once(pinned)
        .chain(std::env::split_paths(&path))
        .find_map(|dir| {
            names
                .iter()
                .map(|name| dir.join(name))
                .find(|candidate| candidate.is_file())
        })
}

fn no_linter() -> String {
    "stratum-lint не найден: установите @petukhovart/stratum-lint в репозитории или глобально (npm i -g @petukhovart/stratum-lint)".to_owned()
}

fn git_command(repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

async fn git(repo: &Path, args: &[&str], limit: Duration) -> Result<String, String> {
    let mut command = git_command(repo, args);
    let run = async { child::spawn(&mut command)?.wait_with_output().await };
    let output = tokio::time::timeout(limit, run)
        .await
        .map_err(|_| format!("git {} не ответил за {} с", args.join(" "), limit.as_secs()))?
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => "git не найден в PATH".to_owned(),
            _ => format!("не удалось запустить git: {error}"),
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = stderr.trim();
    Err(if text.is_empty() {
        format!(
            "git {} завершился с кодом {}",
            args.join(" "),
            output.status
        )
    } else {
        text.to_owned()
    })
}

async fn logged_git(
    log: &Log,
    repo: &Path,
    args: &[&str],
    limit: Duration,
) -> Result<String, String> {
    push(log, format!("$ git {}", args.join(" ")));
    let result = git(repo, args, limit).await;
    match &result {
        Ok(out) => out.lines().take(6).for_each(|line| push(log, line)),
        Err(error) => error.lines().take(12).for_each(|line| push(log, line)),
    }
    result
}

pub async fn stratum_probe(repo: String, target: String) -> Result<Probe, String> {
    let repo = clean(&repo);
    let linter = resolve_linter(&repo).map(|path| path.display().to_string());
    let mut config = repo.join(CONFIG).is_file();
    if !config && !target.is_empty() {
        let spec = format!("refs/remotes/origin/{target}:{CONFIG}");
        config = git(&repo, &["cat-file", "-e", &spec], PROBE_TIMEOUT)
            .await
            .is_ok();
    }
    Ok(Probe { config, linter })
}

pub async fn stratum_progress(
    connector: String,
    project: String,
    number: u64,
) -> Result<Progress, String> {
    let key = merge_request_key(&connector, &project, number);
    let log = locked(&manager().progress).get(&key).cloned();
    Ok(log.map(|log| locked(&log).clone()).unwrap_or_default())
}

fn merge_request_key(connector: &str, project: &str, number: u64) -> String {
    format!("{connector}/{project}!{number}")
}

async fn remote_sha(repo: &Path, log: &Log, reference: &str, what: &str) -> Result<String, String> {
    let out = logged_git(log, repo, &["ls-remote", "origin", reference], GIT_TIMEOUT).await?;
    out.lines()
        .find_map(|line| {
            let (sha, name) = line.split_once(char::is_whitespace)?;
            (name.trim() == reference).then(|| sha.to_owned())
        })
        .ok_or_else(|| format!("{what}: ссылка {reference} не найдена на origin"))
}

fn reference_of(kind: Kind, number: u64) -> String {
    match kind {
        Kind::Github => format!("refs/pull/{number}/head"),
        _ => format!("refs/merge-requests/{number}/head"),
    }
}

fn cache_dir() -> Result<PathBuf, String> {
    Ok(berth_dir()?.join("stratum"))
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(8)]
}

fn repo_name(project: &str) -> String {
    let name = project.rsplit('/').next().unwrap_or(project);
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub async fn stratum_visualize(
    state: &Connectors,
    connector: String,
    project: String,
    number: u64,
    repo: String,
    target: String,
) -> Result<Visualizer, String> {
    let kind = state
        .forge_records()
        .into_iter()
        .find(|record| record.id == connector)
        .map(|record| record.kind)
        .ok_or_else(|| format!("подключение {connector} не найдено"))?;
    let mr = merge_request_key(&connector, &project, number);
    let log: Log = Arc::new(Mutex::new(Progress {
        phase: "queued".to_owned(),
        lines: Vec::new(),
    }));
    locked(&manager().progress).insert(mr.clone(), log.clone());
    let result = start(
        kind,
        &mr,
        &project,
        number,
        clean(&repo),
        target,
        log.clone(),
    )
    .await;
    if let Err(error) = &result {
        push(&log, error.clone());
        phase(&log, "failed");
    }
    result
}

async fn start(
    kind: Kind,
    mr: &str,
    project: &str,
    number: u64,
    repo: PathBuf,
    target: String,
    log: Log,
) -> Result<Visualizer, String> {
    let manager = manager();
    let _turn = manager.starting.lock().await;
    sweep_once().await;

    let linter = resolve_linter(&repo).ok_or_else(no_linter)?;
    phase(&log, "fetch");
    let head_ref = reference_of(kind, number);
    let head = remote_sha(&repo, &log, &head_ref, "запрос слияния").await?;
    let key = format!("{mr}@{}", short(&head));

    if let Some(server) = alive(&key).await {
        *locked(&server.leases) += 1;
        *locked(&server.used) = Instant::now();
        return Ok(visualizer(&server));
    }

    let target_ref = format!("refs/heads/{target}");
    let target_sha = remote_sha(&repo, &log, &target_ref, "целевая ветка").await?;
    logged_git(
        &log,
        &repo,
        &[
            "fetch",
            "--no-write-fetch-head",
            "origin",
            &head,
            &target_sha,
        ],
        GIT_TIMEOUT,
    )
    .await?;
    let base = logged_git(
        &log,
        &repo,
        &["merge-base", &head, &target_sha],
        GIT_TIMEOUT,
    )
    .await
    .map_err(|error| format!("не найдена общая база с {target}: {error}"))?;
    let spec = format!("{head}:{CONFIG}");
    if git(&repo, &["cat-file", "-e", &spec], PROBE_TIMEOUT)
        .await
        .is_err()
    {
        return Err(format!(
            "в ветке запроса {project}!{number} нет {CONFIG}: она создана до его добавления, обновите ветку от {target}"
        ));
    }

    evict_for_room().await;

    phase(&log, "worktree");
    let dir = cache_dir()?.join(format!(
        "{}-mr{number}-{}",
        repo_name(project),
        short(&head)
    ));
    let leftover = dir.clone();
    let leftover_repo = repo.clone();
    let _ = tokio::task::spawn_blocking(move || remove_worktree(&leftover_repo, &leftover)).await;
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let dir_text = dir.to_string_lossy().into_owned();
    logged_git(
        &log,
        &repo,
        &["worktree", "add", "--detach", &dir_text, &head],
        GIT_TIMEOUT,
    )
    .await?;

    phase(&log, "start");
    let spawned = launch(&linter, &dir, &base, &log).await;
    let (child, url) = match spawned {
        Ok(found) => found,
        Err(error) => {
            let (cleanup_repo, cleanup_dir) = (repo.clone(), dir.clone());
            let _ =
                tokio::task::spawn_blocking(move || remove_worktree(&cleanup_repo, &cleanup_dir))
                    .await;
            return Err(error);
        }
    };
    let server = Arc::new(Server {
        key: key.clone(),
        repo,
        dir,
        pid: child.id().unwrap_or_default(),
        url,
        head,
        base,
        child: tokio::sync::Mutex::new(child),
        leases: Mutex::new(1),
        used: Mutex::new(Instant::now()),
    });
    locked(&manager.servers).insert(key, server.clone());
    phase(&log, "ready");
    Ok(visualizer(&server))
}

fn visualizer(server: &Server) -> Visualizer {
    Visualizer {
        key: server.key.clone(),
        url: server.url.clone(),
        head: server.head.clone(),
        base: server.base.clone(),
    }
}

async fn alive(key: &str) -> Option<Arc<Server>> {
    let server = locked(&manager().servers).get(key).cloned()?;
    let running = matches!(server.child.lock().await.try_wait(), Ok(None));
    if running {
        return Some(server);
    }
    locked(&manager().servers).remove(key);
    let dead = server.clone();
    let _ = tokio::task::spawn_blocking(move || stop_blocking(&dead)).await;
    None
}

async fn evict_for_room() {
    loop {
        let victim = {
            let mut servers = locked(&manager().servers);
            if servers.len() < MAX_SERVERS {
                return;
            }
            let oldest = servers
                .values()
                .min_by_key(|server| (*locked(&server.leases) > 0, *locked(&server.used)))
                .map(|server| server.key.clone());
            oldest.and_then(|key| servers.remove(&key))
        };
        let Some(victim) = victim else { return };
        let _ = tokio::task::spawn_blocking(move || stop_blocking(&victim)).await;
    }
}

async fn launch(
    linter: &Path,
    dir: &Path,
    base: &str,
    log: &Log,
) -> Result<(Child, String), String> {
    let mut command = Command::new(linter);
    command
        .arg("visualize")
        .arg(dir)
        .args(["--base", base, "--no-open"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    push(
        log,
        format!(
            "$ stratum-lint visualize <worktree> --base {} --no-open",
            short(base)
        ),
    );
    let mut child = child::spawn(&mut command)
        .map_err(|error| format!("не удалось запустить {}: {error}", linter.display()))?;
    let stderr = child.stderr.take().ok_or("нет потока stderr")?;
    let (found, ready) = oneshot::channel::<String>();
    let sink = log.clone();
    tokio::spawn(async move {
        let mut found = Some(found);
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim_end().to_owned();
            if let Some(url) = line.split(READY_MARK).nth(1) {
                if let Some(found) = found.take() {
                    let _ = found.send(url.trim().to_owned());
                }
            }
            push(&sink, line);
        }
    });
    let outcome = tokio::time::timeout(READY_TIMEOUT, ready).await;
    match outcome {
        Ok(Ok(url)) => Ok((child, url)),
        other => {
            if let Some(pid) = child.id() {
                kill_tree(pid);
            }
            let _ = child.wait().await;
            let tail = locked(log)
                .lines
                .iter()
                .rev()
                .take(8)
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            Err(match other {
                Err(_) => format!(
                    "stratum-lint не запустился за {} с\n{tail}",
                    READY_TIMEOUT.as_secs()
                ),
                _ => format!("stratum-lint завершился без адреса\n{tail}"),
            })
        }
    }
}

pub async fn stratum_release(key: String) -> Result<(), String> {
    let Some(server) = locked(&manager().servers).get(&key).cloned() else {
        return Ok(());
    };
    {
        let mut leases = locked(&server.leases);
        *leases = leases.saturating_sub(1);
        if *leases > 0 {
            return Ok(());
        }
    }
    *locked(&server.used) = Instant::now();
    tokio::spawn(async move {
        tokio::time::sleep(RELEASE_GRACE).await;
        if *locked(&server.leases) > 0 {
            return;
        }
        let removed = {
            let mut servers = locked(&manager().servers);
            match servers.get(&server.key) {
                Some(current) if Arc::ptr_eq(current, &server) => servers.remove(&server.key),
                _ => None,
            }
        };
        if let Some(server) = removed {
            let _ = tokio::task::spawn_blocking(move || stop_blocking(&server)).await;
        }
    });
    Ok(())
}

pub fn shutdown() {
    let servers: Vec<Arc<Server>> = locked(&manager().servers)
        .drain()
        .map(|(_, server)| server)
        .collect();
    for server in &servers {
        kill_tree(server.pid);
    }
    for server in &servers {
        remove_worktree(&server.repo, &server.dir);
    }
}

fn stop_blocking(server: &Server) {
    kill_tree(server.pid);
    remove_worktree(&server.repo, &server.dir);
}

fn kill_tree(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(NO_WINDOW)
            .status();
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn std_git(repo: &Path, args: &[&std::ffi::OsStr]) -> bool {
    let mut command = std::process::Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(NO_WINDOW);
    }
    command.status().is_ok_and(|status| status.success())
}

fn remove_worktree(repo: &Path, dir: &Path) {
    use std::ffi::OsStr;
    if !dir.exists() {
        std_git(repo, &[OsStr::new("worktree"), OsStr::new("prune")]);
        return;
    }
    for attempt in 0..5 {
        let removed = std_git(
            repo,
            &[
                OsStr::new("worktree"),
                OsStr::new("remove"),
                OsStr::new("--force"),
                dir.as_os_str(),
            ],
        );
        if removed || !dir.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(400));
        if attempt >= 2 {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    std_git(repo, &[OsStr::new("worktree"), OsStr::new("prune")]);
}

pub async fn sweep_once() {
    static SWEPT: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
    SWEPT
        .get_or_init(|| async {
            let _ = tokio::task::spawn_blocking(sweep).await;
        })
        .await;
}

fn fold_path(text: &str) -> String {
    text.trim_start_matches(r"\\?\")
        .replace('\\', "/")
        .to_lowercase()
}

fn kill_orphans(root: &Path) {
    let prefix = format!(
        "{}/",
        fold_path(&root.to_string_lossy()).trim_end_matches('/')
    );
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::Always),
    );
    let own = Pid::from_u32(std::process::id());
    let matched: HashSet<Pid> = system
        .processes()
        .iter()
        .filter(|(pid, process)| {
            **pid != own
                && (process
                    .cwd()
                    .is_some_and(|cwd| fold_path(&cwd.to_string_lossy()).starts_with(&prefix))
                    || process
                        .cmd()
                        .iter()
                        .any(|arg| fold_path(&arg.to_string_lossy()).contains(&prefix)))
        })
        .map(|(pid, _)| *pid)
        .collect();
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (pid, process) in system.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(*pid);
        }
    }
    let mut queue: Vec<Pid> = matched
        .iter()
        .filter(|pid| {
            let parent = system.process(**pid).and_then(|process| process.parent());
            match parent {
                Some(parent) if matched.contains(&parent) => false,
                Some(parent) => !system.process(parent).is_some_and(|process| {
                    process
                        .name()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains("berth")
                }),
                None => true,
            }
        })
        .copied()
        .collect();
    let mut doomed: HashSet<Pid> = HashSet::new();
    while let Some(pid) = queue.pop() {
        if pid == own || !doomed.insert(pid) {
            continue;
        }
        queue.extend(children.get(&pid).into_iter().flatten().copied());
    }
    for pid in doomed {
        if let Some(process) = system.process(pid) {
            process.kill();
        }
    }
    if !matched.is_empty() {
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn sweep() {
    let Ok(root) = cache_dir() else { return };
    kill_orphans(&root);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    let live: HashSet<PathBuf> = locked(&manager().servers)
        .values()
        .map(|server| server.dir.clone())
        .collect();
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || live.contains(&dir) {
            continue;
        }
        let repo = std::fs::read_to_string(dir.join(".git"))
            .ok()
            .and_then(|text| {
                let pointer = text.trim().strip_prefix("gitdir:")?.trim().to_owned();
                let admin = PathBuf::from(pointer);
                admin.parent()?.parent().map(Path::to_path_buf)
            });
        match repo {
            Some(common) => remove_worktree(&common, &dir),
            None => {
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }
}
