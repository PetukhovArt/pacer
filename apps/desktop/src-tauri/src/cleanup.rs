use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::task::spawn_blocking;

use crate::connectors::forge::branch_request;
use crate::connectors::types::{BranchRequest, CrState};
use crate::connectors::Connectors;

const QUICK: Duration = Duration::from_secs(20);
const FETCH: Duration = Duration::from_secs(60);
const DAY_MS: i64 = 86_400_000;
const NESTED_GIT_DEPTH: usize = 3;
const DIFF_LIMIT: usize = 512 * 1024;
const BASE_CANDIDATES: [&str; 6] = [
    "origin/main",
    "origin/master",
    "origin/develop",
    "main",
    "master",
    "develop",
];
const IN_PROGRESS: [&str; 4] = [
    "rebase-merge",
    "rebase-apply",
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProject {
    id: String,
    repo: String,
    worktrees: Vec<ScanWorktree>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanWorktree {
    id: String,
    path: String,
    live: bool,
    last_session_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Safe,
    Review,
    Changes,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    RequestMerged,
    RequestClosed,
    Merged,
    BranchGone,
    Stale,
    Missing,
    Active,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeScan {
    id: String,
    path: String,
    branch: Option<String>,
    category: Category,
    reason: Reason,
    live: bool,
    changes: u32,
    untracked: u32,
    in_progress: bool,
    ahead: Option<u32>,
    idle_days: Option<u32>,
    request: Option<BranchRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrphanKind {
    StaleGit,
    Leftover,
}

#[derive(Debug, Serialize)]
pub struct Orphan {
    path: String,
    kind: OrphanKind,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScan {
    id: String,
    repo: String,
    base: Option<String>,
    fetched: bool,
    error: Option<String>,
    worktrees: Vec<WorktreeScan>,
    orphans: Vec<Orphan>,
}

#[derive(Debug, Serialize)]
pub struct LinkCheck {
    link: String,
    target: Option<String>,
    before: Option<u64>,
    after: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct Unlinked {
    links: Vec<LinkCheck>,
    removed: bool,
}

async fn git_run(dir: &Path, args: &[&str], timeout: Duration) -> Option<(bool, String)> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let run = async { crate::child::spawn(&mut command)?.wait_with_output().await };
    match tokio::time::timeout(timeout, run).await {
        Ok(Ok(output)) => {
            let ok = output.status.success();
            let text = if ok { output.stdout } else { output.stderr };
            Some((ok, String::from_utf8_lossy(&text).trim_end().to_owned()))
        }
        _ => None,
    }
}

async fn git_out(dir: &Path, args: &[&str]) -> Option<String> {
    match git_run(dir, args, QUICK).await {
        Some((true, out)) => Some(out),
        _ => None,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn key(path: &Path) -> String {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = resolved.to_string_lossy().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    let text = text.trim_end_matches('/').to_owned();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn is_within(inner: &str, outer: &str) -> bool {
    inner == outer || inner.starts_with(&format!("{outer}/"))
}

fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        meta.file_type().is_symlink() || meta.file_attributes() & REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

async fn detect_base(repo: &Path) -> Option<String> {
    if let Some(head) = git_out(
        repo,
        &["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"],
    )
    .await
    {
        if git_out(repo, &["rev-parse", "-q", "--verify", &head])
            .await
            .is_some()
        {
            return Some(head);
        }
    }
    for candidate in BASE_CANDIDATES {
        if git_out(repo, &["rev-parse", "-q", "--verify", candidate])
            .await
            .is_some()
        {
            return Some(candidate.to_owned());
        }
    }
    None
}

fn registered_worktrees(porcelain: &str) -> Vec<String> {
    porcelain
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(str::to_owned)
        .collect()
}

struct Inputs<'a> {
    base: Option<&'a str>,
    stale_days: u32,
    now: i64,
    state: &'a Connectors,
}

async fn in_progress(path: &Path) -> bool {
    for marker in IN_PROGRESS {
        if let Some(found) = git_out(path, &["rev-parse", "--git-path", marker]).await {
            let found = PathBuf::from(found);
            let full = if found.is_absolute() {
                found
            } else {
                path.join(found)
            };
            if full.exists() {
                return true;
            }
        }
    }
    false
}

async fn branch_gone(path: &Path, branch: &str) -> bool {
    let remote = git_out(path, &["config", &format!("branch.{branch}.remote")]).await;
    let merge = git_out(path, &["config", &format!("branch.{branch}.merge")]).await;
    let (Some(remote), Some(merge)) = (remote, merge) else {
        return false;
    };
    if remote.is_empty() || remote == "." || merge.is_empty() {
        return false;
    }
    let short = merge.strip_prefix("refs/heads/").unwrap_or(&merge);
    let tracking = format!("refs/remotes/{remote}/{short}");
    git_out(path, &["rev-parse", "-q", "--verify", &tracking])
        .await
        .is_none()
}

async fn moved_since_created(path: &Path, branch: &str, head: &str, base: &str) -> bool {
    let reflog = git_out(
        path,
        &[
            "reflog",
            "show",
            "--format=%H",
            &format!("refs/heads/{branch}"),
        ],
    )
    .await
    .unwrap_or_default();
    let entries = reflog.lines().filter(|line| !line.is_empty()).count();
    if entries > 1 {
        return true;
    }
    if entries == 1 {
        return false;
    }
    let base_tip = git_out(path, &["rev-parse", base])
        .await
        .unwrap_or_default();
    base_tip != head
}

fn same_branch(branch: &str, base: &str) -> bool {
    let short = base.split_once('/').map_or(base, |(_, rest)| rest);
    branch == base || branch == short
}

async fn scan_worktree(worktree: &ScanWorktree, inputs: &Inputs<'_>) -> WorktreeScan {
    let path = Path::new(&worktree.path);
    let mut scan = WorktreeScan {
        id: worktree.id.clone(),
        path: worktree.path.clone(),
        branch: None,
        category: Category::Keep,
        reason: Reason::Active,
        live: worktree.live,
        changes: 0,
        untracked: 0,
        in_progress: false,
        ahead: None,
        idle_days: None,
        request: None,
    };
    if !path.exists() {
        scan.category = if worktree.live {
            Category::Review
        } else {
            Category::Safe
        };
        scan.reason = Reason::Missing;
        return scan;
    }
    let Some(head) = git_out(path, &["rev-parse", "HEAD"]).await else {
        return scan;
    };
    scan.branch = git_out(path, &["symbolic-ref", "-q", "--short", "HEAD"])
        .await
        .filter(|b| !b.is_empty());
    let status = git_out(path, &["status", "--porcelain"])
        .await
        .unwrap_or_default();
    for line in status.lines().filter(|line| !line.is_empty()) {
        if line.starts_with("??") {
            scan.untracked += 1;
        } else {
            scan.changes += 1;
        }
    }
    scan.in_progress = in_progress(path).await;
    let last_commit = git_out(path, &["log", "-1", "--format=%ct"])
        .await
        .and_then(|s| s.trim().parse::<i64>().ok())
        .map_or(0, |s| s * 1000);
    let last = last_commit.max(worktree.last_session_ms);
    if last > 0 {
        scan.idle_days = Some(((inputs.now - last).max(0) / DAY_MS) as u32);
    }
    let mut finished = None;
    if let Some(branch) = scan.branch.clone() {
        let on_base = inputs.base.is_some_and(|base| same_branch(&branch, base));
        scan.request = branch_request(inputs.state, &worktree.path, &branch)
            .await
            .ok()
            .flatten()
            .filter(|_| !on_base);
        if let Some(base) = inputs.base {
            scan.ahead = git_out(path, &["rev-list", "--count", &format!("{base}..HEAD")])
                .await
                .and_then(|s| s.trim().parse().ok());
        }
        finished = match scan.request.as_ref().map(|r| r.state) {
            Some(CrState::Merged) => Some(Reason::RequestMerged),
            Some(CrState::Closed) => Some(Reason::RequestClosed),
            Some(CrState::Open) => None,
            None if on_base => None,
            None => {
                let merged = match inputs.base {
                    Some(base) if scan.ahead == Some(0) => {
                        moved_since_created(path, &branch, &head, base).await
                    }
                    _ => false,
                };
                if merged {
                    Some(Reason::Merged)
                } else if branch_gone(path, &branch).await {
                    Some(Reason::BranchGone)
                } else {
                    None
                }
            }
        };
    }
    let dirty = scan.changes > 0 || scan.untracked > 0 || scan.in_progress;
    let stale = scan.idle_days.is_some_and(|days| days >= inputs.stale_days);
    if let Some(reason) = finished {
        scan.reason = reason;
        scan.category = if dirty {
            Category::Changes
        } else if worktree.live {
            Category::Review
        } else {
            Category::Safe
        };
    } else if stale && !worktree.live {
        scan.reason = Reason::Stale;
        scan.category = if dirty {
            Category::Changes
        } else {
            Category::Review
        };
    }
    scan
}

fn nested_git(dir: &Path, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.path().symlink_metadata() else {
            continue;
        };
        if is_link(&meta) || !meta.is_dir() {
            if entry.file_name() == ".git" {
                return true;
            }
            continue;
        }
        if entry.file_name() == ".git" || nested_git(&entry.path(), depth - 1) {
            return true;
        }
    }
    false
}

enum Child {
    Skip,
    Repo,
    StaleGit,
    Plain,
}

fn classify(dir: &Path, common: &str) -> Child {
    let dot_git = dir.join(".git");
    match dot_git.symlink_metadata() {
        Ok(meta) if meta.is_dir() => Child::Repo,
        Ok(meta) if meta.is_file() => {
            let Ok(text) = fs::read_to_string(&dot_git) else {
                return Child::Skip;
            };
            let Some(pointer) = text
                .lines()
                .find_map(|line| line.strip_prefix("gitdir:"))
                .map(str::trim)
            else {
                return Child::Skip;
            };
            let pointer = PathBuf::from(pointer);
            let gitdir = if pointer.is_absolute() {
                pointer
            } else {
                dir.join(pointer)
            };
            let owner = gitdir.parent().and_then(Path::parent).map(key);
            if owner.as_deref() == Some(common) {
                Child::StaleGit
            } else {
                Child::Skip
            }
        }
        Ok(_) => Child::Skip,
        Err(_) if nested_git(dir, NESTED_GIT_DEPTH) => Child::Repo,
        Err(_) => Child::Plain,
    }
}

fn child_dirs(container: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(container) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.symlink_metadata()
                .is_ok_and(|meta| meta.is_dir() && !is_link(&meta))
        })
        .collect()
}

struct Containers {
    dirs: Vec<(PathBuf, bool)>,
    sibling_layout: bool,
}

async fn containers_of(repo: &Path, worktree_paths: &[String]) -> Containers {
    let repo_key = key(repo);
    let parent = repo.parent().map(key);
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();
    let mut sibling_layout = false;
    for path in worktree_paths {
        let Some(container) = Path::new(path).parent() else {
            continue;
        };
        let container_key = key(container);
        if parent.as_deref() == Some(container_key.as_str()) {
            sibling_layout = true;
            continue;
        }
        if is_within(&repo_key, &container_key) || container.parent().is_none() {
            continue;
        }
        if !seen.insert(container_key.clone()) {
            continue;
        }
        let leftovers = if is_within(&container_key, &repo_key) {
            let spelled = container.to_string_lossy().into_owned();
            matches!(
                git_run(repo, &["check-ignore", "-q", &spelled], QUICK).await,
                Some((true, _))
            )
        } else {
            true
        };
        dirs.push((container.to_path_buf(), leftovers));
    }
    Containers {
        dirs,
        sibling_layout,
    }
}

fn find_orphans(
    repo: &Path,
    common: &str,
    containers: Containers,
    known: &HashSet<String>,
) -> Vec<Orphan> {
    let sibling_layout = containers.sibling_layout;
    let mut out = Vec::new();
    for (container, leftovers) in containers.dirs {
        let children: Vec<(PathBuf, Child)> = child_dirs(&container)
            .into_iter()
            .filter(|dir| !known.contains(&key(dir)))
            .map(|dir| {
                let kind = classify(&dir, common);
                (dir, kind)
            })
            .collect();
        let dedicated = leftovers && !children.iter().any(|(_, kind)| matches!(kind, Child::Repo));
        for (dir, kind) in children {
            let kind = match kind {
                Child::StaleGit => OrphanKind::StaleGit,
                Child::Plain if dedicated => OrphanKind::Leftover,
                _ => continue,
            };
            out.push(Orphan {
                path: dir.to_string_lossy().into_owned(),
                kind,
            });
        }
    }
    if sibling_layout {
        if let (Some(parent), Some(name)) = (repo.parent(), repo.file_name()) {
            let prefix = format!("{}-", name.to_string_lossy());
            for dir in child_dirs(parent) {
                let named = dir
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(&prefix));
                if !named || known.contains(&key(&dir)) {
                    continue;
                }
                if matches!(classify(&dir, common), Child::StaleGit) {
                    out.push(Orphan {
                        path: dir.to_string_lossy().into_owned(),
                        kind: OrphanKind::StaleGit,
                    });
                }
            }
        }
    }
    out
}

async fn scan_project(
    state: &Connectors,
    project: &ScanProject,
    stale_days: u32,
    known: &HashSet<String>,
) -> ProjectScan {
    let repo = Path::new(&project.repo);
    let mut scan = ProjectScan {
        id: project.id.clone(),
        repo: project.repo.clone(),
        base: None,
        fetched: false,
        error: None,
        worktrees: Vec::new(),
        orphans: Vec::new(),
    };
    let Some(common) = git_out(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await
    else {
        scan.error = Some("not a git repository".to_owned());
        return scan;
    };
    let common = key(Path::new(&common));
    let remotes = git_out(repo, &["remote"]).await.unwrap_or_default();
    if !remotes.trim().is_empty() {
        scan.fetched = matches!(
            git_run(repo, &["fetch", "--all", "--prune", "--quiet"], FETCH).await,
            Some((true, _))
        );
    }
    scan.base = detect_base(repo).await;
    let registered = git_out(repo, &["worktree", "list", "--porcelain"])
        .await
        .map(|text| registered_worktrees(&text))
        .unwrap_or_default();
    let inputs = Inputs {
        base: scan.base.as_deref(),
        stale_days,
        now: now_ms(),
        state,
    };
    let repo_key = key(repo);
    let worktrees: Vec<&ScanWorktree> = project
        .worktrees
        .iter()
        .filter(|w| key(Path::new(&w.path)) != repo_key)
        .collect();
    scan.worktrees = join_all(worktrees.iter().map(|w| scan_worktree(w, &inputs))).await;
    let mut paths: Vec<String> = worktrees.iter().map(|w| w.path.clone()).collect();
    paths.extend(registered.iter().cloned());
    let mut known = known.clone();
    known.extend(registered.iter().map(|p| key(Path::new(p))));
    let containers = containers_of(repo, &paths).await;
    let repo_owned = repo.to_path_buf();
    scan.orphans = spawn_blocking(move || find_orphans(&repo_owned, &common, containers, &known))
        .await
        .unwrap_or_default();
    scan
}

pub async fn cleanup_scan(
    state: &Connectors,
    projects: Vec<ScanProject>,
    stale_days: u32,
) -> Result<Vec<ProjectScan>, String> {
    let mut known: HashSet<String> = HashSet::new();
    for project in &projects {
        known.insert(key(Path::new(&project.repo)));
        for worktree in &project.worktrees {
            known.insert(key(Path::new(&worktree.path)));
        }
    }
    Ok(join_all(
        projects
            .iter()
            .map(|project| scan_project(state, project, stale_days, &known)),
    )
    .await)
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(meta) = entry.path().symlink_metadata() else {
            continue;
        };
        if is_link(&meta) {
            continue;
        }
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else {
            total += meta.len();
        }
    }
    total
}

pub async fn cleanup_sizes(paths: Vec<String>) -> Result<Vec<u64>, String> {
    spawn_blocking(move || paths.iter().map(|p| dir_size(Path::new(p))).collect())
        .await
        .map_err(|e| e.to_string())
}

fn collect_links(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let meta = path.symlink_metadata()?;
        if is_link(&meta) {
            out.push(path);
        } else if meta.is_dir() {
            collect_links(&path, out)?;
        }
    }
    Ok(())
}

fn entry_count(path: &Path) -> Option<u64> {
    fs::read_dir(path)
        .ok()
        .map(|entries| entries.count() as u64)
}

fn unlink(path: &Path) -> std::io::Result<()> {
    fs::remove_dir(path).or_else(|_| fs::remove_file(path))
}

fn guard(repo: &Path, path: &Path) -> Result<(), String> {
    let target = key(path);
    let repo_key = key(repo);
    if path.parent().is_none() || target.is_empty() {
        return Err(format!("refusing to touch {}", path.display()));
    }
    if is_within(&repo_key, &target) || is_within(&target, &repo_key) {
        return Err(format!(
            "refusing to touch {}: it is or holds the main checkout",
            path.display()
        ));
    }
    Ok(())
}

fn unlink_tree(path: &Path, remove: bool) -> Result<Unlinked, String> {
    let mut links = Vec::new();
    collect_links(path, &mut links).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut checks = Vec::new();
    for link in &links {
        let target = fs::read_link(link).ok();
        let before = target.as_deref().and_then(entry_count);
        unlink(link).map_err(|e| format!("{}: {e}", link.display()))?;
        checks.push((link.clone(), target, before));
    }
    let mut left = Vec::new();
    collect_links(path, &mut left).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(first) = left.first() {
        return Err(format!("a link is still in place: {}", first.display()));
    }
    let links: Vec<LinkCheck> = checks
        .into_iter()
        .map(|(link, target, before)| LinkCheck {
            after: target.as_deref().and_then(entry_count),
            link: link.to_string_lossy().into_owned(),
            target: target.map(|t| t.to_string_lossy().into_owned()),
            before,
        })
        .collect();
    if let Some(lost) = links
        .iter()
        .find(|c| c.before.is_some() && c.after < c.before)
    {
        return Err(format!(
            "{} lost entries after unlinking {}",
            lost.target.as_deref().unwrap_or("?"),
            lost.link
        ));
    }
    if remove {
        fs::remove_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(Unlinked {
        links,
        removed: remove,
    })
}

pub async fn cleanup_unlink(repo: String, path: String, remove: bool) -> Result<Unlinked, String> {
    let repo = PathBuf::from(repo);
    let path = PathBuf::from(path);
    guard(&repo, &path)?;
    if !path.exists() {
        return Ok(Unlinked {
            links: Vec::new(),
            removed: false,
        });
    }
    spawn_blocking(move || unlink_tree(&path, remove))
        .await
        .map_err(|e| e.to_string())?
}

fn valid_branch(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.contains("..")
        && !name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c))
}

pub async fn cleanup_finish(
    repo: String,
    branch: Option<String>,
    delete_branch: bool,
) -> Result<bool, String> {
    let repo = PathBuf::from(repo);
    let _ = git_run(&repo, &["worktree", "prune"], QUICK).await;
    let Some(branch) = branch.filter(|b| delete_branch && valid_branch(b)) else {
        return Ok(false);
    };
    let current = git_out(&repo, &["symbolic-ref", "-q", "--short", "HEAD"]).await;
    if current.as_deref() == Some(branch.as_str()) {
        return Err(format!("{branch} is checked out in the main checkout"));
    }
    match git_run(&repo, &["branch", "-D", &branch], QUICK).await {
        Some((true, _)) => Ok(true),
        Some((false, message)) => Err(message),
        None => Err("git branch -D timed out".to_owned()),
    }
}

pub async fn cleanup_diff(path: String) -> Result<String, String> {
    let dir = PathBuf::from(&path);
    let status = git_out(&dir, &["status", "--short"])
        .await
        .ok_or_else(|| format!("git status failed in {path}"))?;
    let diff = git_out(&dir, &["diff", "HEAD", "--no-color", "--no-ext-diff"])
        .await
        .unwrap_or_default();
    let mut text = format!("{status}\n\n{diff}");
    if text.len() > DIFF_LIMIT {
        let mut end = DIFF_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n…");
    }
    Ok(text)
}
