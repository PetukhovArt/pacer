<div align="center">

# pacer

**Where your coding agents live.**

Run **Claude Code**, **Codex** and **Cursor** across every project and git worktree you own — from one
terminal, one keyboard, one tree. They keep working when you close it.

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)](#install)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-dea584?style=flat-square)](https://www.rust-lang.org)

<img src="assets/screenshot.png" alt="pacer: projects, worktrees and sessions on the left, a live Claude Code session on the right" width="100%">

</div>

---

## Why pacer

You start three agents in three terminal tabs. Five minutes later you have no idea which one is waiting
on a permission prompt, which one finished, and which one is still thinking — so you tab through all
three, every time, and read the screens.

pacer replaces that with a tree and a color. Every project, worktree and agent is a row; a dot on the row
says what that agent is doing, and parents roll up their children, so a red dot on a collapsed project
tells you where to look without opening anything. A daemon owns the PTYs, so quitting the UI doesn't stop
the work.

No Electron, no server, no MCP. One ~4 MB Rust binary and a local socket.

| Dot | Meaning |
|---|---|
| ● gray | fresh — agent never run |
| ● yellow | running — turn in progress |
| ● violet | done, unread |
| ● green | done, read |
| ● red | needs feedback — permission prompt or question waiting on you |
| ● magenta | terminated — process died mid-run |
| ○ | disconnected — daemon restarted while the agent was live |

## Key features

- **One tree for everything.** Workspaces → projects → worktrees → sessions, up to four columns,
  `h`/`j`/`k`/`l` to move, `Enter` to drill in.
- **Agents that outlive the UI.** A detached daemon owns every PTY. Quit pacer, close the laptop lid,
  come back tomorrow — the sessions are where you left them, scrollback replayed.
- **Real git worktrees, one keystroke.** Two agents in two directories don't collide. Or just tell a
  session "do this in a worktree" and it moves itself into one.
- **Claude, Codex and Cursor**, each with its model and reasoning effort — plus **agent presets** that
  launch a session already working on a task, and **Claude Cloud** sessions mirrored into a local pane.
- **Pull requests in the same tree.** GitHub (`gh`) and GitLab (`glab`, self-hosted included): open PRs
  under each project with review and pipeline status, the whole conversation as a thread tree in the
  preview pane, `g` for the diff, `n` for an agent scoped to that PR.
- **Read the code without leaving.** Diff viewer with reviewed-file bookkeeping (`g`), fuzzy file find
  (`f`), `git grep` (`F`), a file tree with preview (`b`) — and any of them opens the file in an editor.
- **Lists that stay usable.** Pins (`p`), per-column sorting (`Shift+S`), an inline filter (`Ctrl+F`) and
  a fuzzy jump (`/`) that reaches across every workspace.
- **Runs where you do.** macOS, Linux and Windows natively; over ssh (`pacer ssh`), or in a browser tab
  through one tunnel (`pacer tunnel`) — including from a phone.

## Install

**npm** — macOS, Linux and Windows alike. The package is a launcher; npm fetches the binary for your
platform only:

```sh
npm install -g @petukhovart/pacer
```

**macOS / Linux** — the same command installs and updates:

```sh
curl -fsSL https://raw.githubusercontent.com/PetukhovArt/pacer/main/install.sh | sh
```

It downloads the prebuilt binary for your platform from the latest GitHub release into `~/.local/bin`
(override with `PACER_INSTALL_DIR`), falling back to `cargo install --git` when no release matches.
Afterwards, `pacer upgrade` runs that same script for you.

**From source** — any platform, needs a Rust toolchain:

```sh
cargo install --git https://github.com/PetukhovArt/pacer pacer --locked
```

Either way the command is `pacer`. On Windows, see [docs/windows.md](docs/windows.md) for what to install
alongside it and what differs on the platform.

> **Prerequisite:** at least one agent CLI on your `PATH` — `claude`, `codex` or `cursor-agent`. pacer
> spawns them; it doesn't ship them. `gh` and/or `glab` unlock the pull-request views.

## Quickstart

```sh
pacer add ~/code/my-app   # or `pacer add .` from inside the repo
pacer                     # launch the TUI; the daemon auto-starts
```

Four columns, left to right: **Projects → Worktrees → Sessions → Terminal**. `h`/`j`/`k`/`l` or the arrows
move, `Enter` drills in, and `n` creates whatever the focused column holds — a project, a worktree, a
session. `Ctrl+q` leaves the terminal pane, `q` quits pacer, `?` lists every key. The agents keep running
either way.

Then read [docs/getting-started.md](docs/getting-started.md).

## Documentation

| | |
|---|---|
| [Getting started](docs/getting-started.md) | the full walkthrough: worktrees, sessions, status dots, pull requests, views |
| [Keymap](docs/keymap.md) | every default key, and where it applies |
| [Commands](docs/cli.md) | the CLI: daemon, workspaces, ssh, browser, tunnel |
| [Configuration](docs/configuration.md) | `config.json`, paths, environment variables |
| [Windows](docs/windows.md) | install, requirements, platform differences |
| [Remote access](docs/remote-access-tailscale.md) | reaching the TUI from a phone over Tailscale |
| [Changelog](CHANGELOG.md) | what changed |

## License

MIT — see [LICENSE](LICENSE).

<div align="center">
<br>
<sub>If pacer saves you a tab, a ⭐ helps other people find it.</sub>
</div>
