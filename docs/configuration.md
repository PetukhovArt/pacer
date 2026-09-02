# Configuration

Settings live in `config.json`, beside the database. It is read fresh on each use, so hand edits apply
without a restart — and `s` in the TUI is an overlay over the same file.

## Where things live

| | Path |
|---|---|
| macOS | `~/Library/Application Support/dev.nebula.nebula/` |
| Linux | `~/.local/share/nebula/` (data), `~/.local/state/nebula/` (logs) |
| Windows | `%APPDATA%\nebula\nebula\data\` |

The directory is still called `nebula` — the project's old name, kept on purpose. Renaming it would
strand every existing install's sessions, pins and settings in a directory nothing reads any more. Same
for `nebula.db` and the runtime dir.

That directory holds `config.json`, `agent_presets.json`, `nebula.db`, and a `state/` subdirectory with
`daemon.log` and `tui.log`.

## Settings

Everything below is both a key in `config.json` and a row in the `s` overlay.

| Key | Default | What it does |
|---|---|---|
| `theme` | `default` | color theme |
| `animations` | `true` | status-text sweep and splash motion |
| `focus_tint` | `false` | tint the focused panel |
| `editor` | `vim` | what the `f` / `F` / `b` modals launch (`vim`, `nvim`, `nano`, `emacs`, `hx`, or any command line — it is called as `<editor> +<line> <file>`) |
| `done_sound` | `Glass` | what rings when a turn finishes: `off`, `bell`, or a macOS system sound |
| `session_idle_timeout` | `5m` | how long an unwatched idle session keeps its PTY before it is reaped (`off` never reaps) |
| `skip_session_naming` | `false` | skip the name prompt when creating a session |
| `palette_enter_attaches` | `true` | whether `Enter` in the fuzzy jump attaches or only selects |
| `git_init_on_create` | — | `git init` a non-repo directory when it is added |
| `pr_list_filter` | `all` | which open pull requests the group lists: `all`, `mine`, `involved` |
| `sort_projects` / `sort_worktrees` / `sort_sessions` | `created` | per-column order: `created`, `recent` or `name` (pins float first) |
| `show_workspaces` | | show the Workspaces bar |
| `hide_projects` / `hide_worktrees` | `false` | start with that sidebar hidden; the Sessions panel always stays visible |
| `claude_enabled` / `codex_enabled` / `cursor_enabled` | `true` | which harnesses the new-session menu offers |
| `claude_model` / `claude_effort` / `codex_model` / `codex_effort` | | defaults the picker takes on `Enter` |
| `keybindings` | | overrides for any action in [keymap.md](keymap.md), written by Settings → Hotkeys |

## Environment

| Variable | Effect |
|---|---|
| `NEBULA_EDITOR` | editor command, ahead of the `editor` setting |
| `NEBULA_LOG` | `RUST_LOG`-style filter for both daemon and TUI (`NEBULA_LOG=debug`) |
| `NEBULA_CLOUD_MIRROR_SECS` | cloud-mirror refresh cadence in seconds; `0` turns the follow off |
| `NEBULA_DATA_DIR` | data dir (database, config, logs) — how tests and parallel instances isolate themselves |
| `NEBULA_RUNTIME_DIR` | runtime dir holding the socket / endpoint file and pidfile |
| `NEBULA_AGENT_CMD` | replace every agent CLI with one command line, taken verbatim (`make dev AGENT=/bin/cat`) |
| `NEBULA_INSTALL_URL` | install-script URL used by `pacer upgrade` and `pacer ssh` |
| `NEBULA_INSTALL_DIR` | install destination for `install.sh` (default `~/.local/bin`) |

Agent PTYs additionally carry `NEBULA_AGENT_ID`, `NEBULA_API_URL` and `NEBULA_API_TOKEN` so hooks and
`pacer rename` / `worktree` / `spawn` can find their own session. They are scrubbed from plain terminals.
