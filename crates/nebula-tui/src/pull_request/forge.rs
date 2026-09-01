//! Which forge a checkout talks to, read off its git remote — the fact
//! every [`super`] call dispatches on, worked out fresh per call: a config
//! read (`git remote get-url`) costs milliseconds next to the network
//! request that follows it, and never goes stale when a remote changes.
//!
//! The host names the forge. `github.com` is GitHub; a host with `gitlab`
//! in it is GitLab; and a self-hosted GitLab whose name says nothing
//! (`git.company.local`) is recognized by being in `glab`'s own config —
//! the user logged `glab` into it, which is also exactly the condition
//! under which asking `glab` about it can work. Everything else defaults
//! to GitHub, which is the pre-GitLab behavior: `gh` gets asked, fails,
//! and the row stays quietly empty.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Forge {
    GitHub,
    GitLab,
}

/// The forge behind `dir`'s remote. No remote, or a remote with no web
/// page, is GitHub — the default that preserves old behavior.
pub(super) async fn detect(dir: &Path) -> Forge {
    match remote_host(dir).await {
        Some(host) => for_host(&host, glab_hosts()),
        None => Forge::GitHub,
    }
}

/// The host of `dir`'s web-facing remote: `origin`'s when it exists, else
/// the first remote git lists — the same guess `remote::repo_url` makes.
/// Also what a per-host cache keys on ([`super::Viewers`]).
pub(super) async fn remote_host(dir: &Path) -> Option<String> {
    let url = match git(dir, &["remote", "get-url", "origin"]).await {
        Some(url) => url,
        None => {
            let names = git(dir, &["remote"]).await?;
            let first = names
                .lines()
                .map(str::trim)
                .find(|n| !n.is_empty())?
                .to_string();
            git(dir, &["remote", "get-url", &first]).await?
        }
    };
    let page = crate::remote::web_url(url.trim())?;
    let rest = page.split_once("://")?.1;
    let host = rest.split('/').next()?;
    Some(strip_port(host).to_ascii_lowercase())
}

async fn git(dir: &Path, args: &[&str]) -> Option<String> {
    super::run("git", Some(dir), args, super::TIMEOUT).await
}

/// `git.lan:3000` answers on a port, but `glab` configures it by name.
fn strip_port(host: &str) -> &str {
    match host.rsplit_once(':') {
        Some((h, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => h,
        _ => host,
    }
}

/// Name the forge for a bare lowercase host. `glab_hosts` are the hosts
/// the local `glab` is configured for.
fn for_host(host: &str, glab_hosts: &[String]) -> Forge {
    if host == "github.com" || host.ends_with(".github.com") {
        return Forge::GitHub;
    }
    if host.contains("gitlab") || glab_hosts.iter().any(|h| h == host) {
        return Forge::GitLab;
    }
    Forge::GitHub
}

/// The hosts in `glab`'s config file, read once per process. A missing or
/// unreadable config is an empty list, not an error — it just means only
/// name-recognizable GitLab hosts are detected.
fn glab_hosts() -> &'static [String] {
    static HOSTS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    HOSTS.get_or_init(|| {
        config_text()
            .map(|text| parse_hosts(&text))
            .unwrap_or_default()
    })
}

/// The text of `glab`'s `config.yml`, from the first place it exists:
/// `GLAB_CONFIG_DIR`, then the platform config dirs `glab` itself uses
/// (`%LOCALAPPDATA%\glab-cli` on Windows, `~/.config/glab-cli` elsewhere).
fn config_text() -> Option<String> {
    let candidates = [
        std::env::var("GLAB_CONFIG_DIR").ok(),
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|d| format!("{d}\\glab-cli")),
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(|d| format!("{d}/glab-cli")),
        std::env::var("HOME")
            .ok()
            .map(|d| format!("{d}/.config/glab-cli")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find_map(|dir| std::fs::read_to_string(Path::new(&dir).join("config.yml")).ok())
}

/// The host keys under the top-level `hosts:` block of `glab`'s YAML,
/// lowercased. A hand-rolled scan rather than a YAML dependency: the keys
/// are the lines at the block's first indent level ending with `:`, and
/// that shape is stable across every `glab` that writes the file.
fn parse_hosts(text: &str) -> Vec<String> {
    let mut hosts = Vec::new();
    let mut in_block = false;
    let mut indent: Option<usize> = None;
    for line in text.lines() {
        let trimmed = line.trim_end();
        if !in_block {
            in_block = trimmed == "hosts:";
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let depth = trimmed.len() - trimmed.trim_start().len();
        if depth == 0 {
            break; // next top-level key ends the block
        }
        let level = *indent.get_or_insert(depth);
        if depth == level {
            if let Some(host) = trimmed.trim().strip_suffix(':') {
                hosts.push(host.to_ascii_lowercase());
            }
        }
    }
    hosts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_come_out_of_a_real_shaped_config() {
        let hosts = parse_hosts(
            "\
git_protocol: ssh
editor:
hosts:
    gitlab.com:
        token: xxx
        git_protocol: ssh
    git.vipaks.local:
        token: yyy
        api_protocol: http

check_update: true
",
        );
        assert_eq!(hosts, ["gitlab.com", "git.vipaks.local"]);
    }

    #[test]
    fn a_config_without_hosts_is_empty_not_an_error() {
        assert!(parse_hosts("git_protocol: ssh\n").is_empty());
        assert!(parse_hosts("").is_empty());
    }

    #[test]
    fn hosts_name_their_forge() {
        let glab: Vec<String> = vec!["git.vipaks.local".into()];
        assert_eq!(for_host("github.com", &glab), Forge::GitHub);
        assert_eq!(for_host("gist.github.com", &glab), Forge::GitHub);
        assert_eq!(for_host("gitlab.com", &[]), Forge::GitLab);
        assert_eq!(for_host("gitlab.company.dev", &[]), Forge::GitLab);
        assert_eq!(
            for_host("git.vipaks.local", &glab),
            Forge::GitLab,
            "a nameless self-hosted GitLab is known by glab's config"
        );
        assert_eq!(
            for_host("git.unknown.local", &glab),
            Forge::GitHub,
            "everything else keeps the old behavior"
        );
    }

    #[test]
    fn ports_do_not_hide_the_host() {
        assert_eq!(strip_port("git.lan:3000"), "git.lan");
        assert_eq!(strip_port("git.lan"), "git.lan");
        assert_eq!(strip_port("git.lan:"), "git.lan:");
    }
}
