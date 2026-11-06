use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;

use crate::connectors::forge::parse_remote;
use crate::connectors::types::Fork;

const FETCH_TIMEOUT: Duration = Duration::from_secs(180);
const QUICK_TIMEOUT: Duration = Duration::from_secs(15);
const REMOTE_TRIES: usize = 5;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchProblem {
    BranchMissing,
    NoCloneUrl,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct Fetched {
    pub remote: String,
    pub base: Option<String>,
    pub problem: Option<FetchProblem>,
    pub detail: Option<String>,
    pub diverged: bool,
}

struct Run {
    ok: bool,
    stdout: String,
    stderr: String,
}

async fn git(repo: &str, args: &[&str], timeout: Duration) -> Result<Run, String> {
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
    let run = async { crate::child::spawn(&mut command)?.wait_with_output().await };
    match tokio::time::timeout(timeout, run).await {
        Ok(Ok(output)) => Ok(Run {
            ok: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Err("git not found in PATH".to_owned())
        }
        Ok(Err(error)) => Err(format!("git: {error}")),
        Err(_) => Err(format!("git {} timed out", args.first().unwrap_or(&""))),
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.contains("..")
        && !name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\".contains(c))
}

fn remote_name(owner: &str) -> String {
    let cleaned: String = owner
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c| c == '-' || c == '.').to_owned();
    if cleaned.is_empty() {
        "fork".to_owned()
    } else {
        cleaned
    }
}

fn same_repo(a: &str, b: &str) -> bool {
    match (parse_remote(a), parse_remote(b)) {
        (Some((host_a, path_a)), Some((host_b, path_b))) => {
            host_a == host_b && path_a.eq_ignore_ascii_case(&path_b)
        }
        _ => false,
    }
}

async fn remote_url(repo: &str, name: &str) -> Result<Option<String>, String> {
    let run = git(repo, &["remote", "get-url", name], QUICK_TIMEOUT).await?;
    Ok(run.ok.then_some(run.stdout))
}

async fn fork_remote(repo: &str, fork: &Fork) -> Result<Result<String, Fetched>, String> {
    let origin = remote_url(repo, "origin").await?.unwrap_or_default();
    let http = origin.starts_with("http://") || origin.starts_with("https://");
    let preferred = if http { &fork.http_url } else { &fork.ssh_url };
    let fallback = if http { &fork.ssh_url } else { &fork.http_url };
    let url = if preferred.is_empty() {
        fallback
    } else {
        preferred
    };
    let base = remote_name(&fork.owner);
    if url.is_empty() {
        return Ok(Err(problem(base, FetchProblem::NoCloneUrl, None)));
    }
    for attempt in 0..REMOTE_TRIES {
        let name = match attempt {
            0 => base.clone(),
            1 => format!("{base}-fork"),
            n => format!("{base}-fork-{n}"),
        };
        match remote_url(repo, &name).await? {
            Some(existing)
                if same_repo(&existing, &fork.http_url) || same_repo(&existing, &fork.ssh_url) =>
            {
                return Ok(Ok(name));
            }
            Some(_) => continue,
            None => {
                let added = git(repo, &["remote", "add", &name, url], QUICK_TIMEOUT).await?;
                if !added.ok {
                    return Ok(Err(problem(name, FetchProblem::Failed, Some(added.stderr))));
                }
                return Ok(Ok(name));
            }
        }
    }
    Ok(Err(problem(
        base,
        FetchProblem::Failed,
        Some("no free remote name for the fork".to_owned()),
    )))
}

fn problem(remote: String, kind: FetchProblem, detail: Option<String>) -> Fetched {
    Fetched {
        remote,
        base: None,
        problem: Some(kind),
        detail,
        diverged: false,
    }
}

pub async fn mr_fetch_source(
    repo: String,
    branch: String,
    local: String,
    fork: Option<Fork>,
) -> Result<Fetched, String> {
    if !valid_name(&branch) {
        return Err(format!("invalid branch name: {branch}"));
    }
    let remote = match &fork {
        Some(fork) => match fork_remote(&repo, fork).await? {
            Ok(name) => name,
            Err(failed) => return Ok(failed),
        },
        None => "origin".to_owned(),
    };
    let refspec = format!("+refs/heads/{branch}:refs/remotes/{remote}/{branch}");
    let run = git(
        &repo,
        &["fetch", "--no-tags", &remote, &refspec],
        FETCH_TIMEOUT,
    )
    .await?;
    if !run.ok {
        let kind = if run.stderr.contains("couldn't find remote ref") {
            FetchProblem::BranchMissing
        } else {
            FetchProblem::Failed
        };
        return Ok(problem(remote, kind, Some(run.stderr)));
    }
    let base = format!("refs/remotes/{remote}/{branch}");
    let diverged = if valid_name(&local) {
        settle_local(&repo, &local, &base).await?
    } else {
        false
    };
    Ok(Fetched {
        base: Some(base),
        remote,
        problem: None,
        detail: None,
        diverged,
    })
}

async fn settle_local(repo: &str, local: &str, base: &str) -> Result<bool, String> {
    let head = format!("refs/heads/{local}");
    let current = git(
        repo,
        &["rev-parse", "--verify", "--quiet", &head],
        QUICK_TIMEOUT,
    )
    .await?;
    if !current.ok {
        return Ok(false);
    }
    let behind = git(
        repo,
        &["merge-base", "--is-ancestor", &head, base],
        QUICK_TIMEOUT,
    )
    .await?;
    if !behind.ok {
        return Ok(true);
    }
    let moved = git(
        repo,
        &["update-ref", &head, base, &current.stdout],
        QUICK_TIMEOUT,
    )
    .await?;
    Ok(!moved.ok)
}
