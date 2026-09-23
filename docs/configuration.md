# Configuration

Settings live in `config.json`, beside the database. It is read fresh on each use, so hand edits apply
without a restart — and `s` in the TUI is an overlay over the same file.

## Where things live

| | Path |
|---|---|
| macOS | `~/Library/Application Support/dev.pacer/` |
| Linux | `~/.local/share/pacer/` (data), `~/.local/state/pacer/` (logs) |
| Windows | `%APPDATA%\pacer\data\` |

That directory holds `config.json`, `agent_presets.json`, `pacer.db`, and a `state/` subdirectory with
`daemon.log` and `tui.log`.

## Settings

Everything below is both a key in `config.json` and a row in the `s` overlay.

| Key | Default | What it does |
|---|---|---|
| `theme` | `default` | color theme |
| `animations` | `true` | status-text sweep and splash motion |
| `focus_tint` | `false` | tint the focused panel |
| `mermaid_renderer` | `mermaid-ascii -f -` | command the `b` preview renders mermaid diagrams with (`.mmd` files and fenced `mermaid` blocks in markdown); the block goes to its stdin, the text art comes back on stdout; empty turns it off |
| `editor` | `vim` | what the `f` / `F` / `b` modals launch (`vim`, `nvim`, `nano`, `emacs`, `hx`, or any command line — it is called as `<editor> +<line> <file>`) |
| `done_sound` | `Glass` | what rings when a turn finishes: `off`, `bell`, or a macOS system sound |
| `session_idle_timeout` | `5m` | how long an unwatched idle session keeps its PTY before it is reaped (`off` never reaps) |
| `skip_session_naming` | `false` | skip the name prompt when creating a session |
| `palette_enter_attaches` | `true` | whether `Enter` in the fuzzy jump attaches or only selects |
| `git_init_on_create` | — | `git init` a non-repo directory when it is added |
| `pr_list_filter` | `all` | which open pull requests the group lists: `all`, `mine`, `involved` |
| `hide_draft_prs` | `false` | leave draft pull requests (and GitLab draft merge requests) out of the PRs panel and `/` |
| `sort_projects` / `sort_worktrees` / `sort_sessions` | `created` | per-column order: `created`, `recent` or `name` (pins float first) |
| `show_ago` | `true` | show the dim `2h ago` label on project, worktree and session rows |
| `bold_names` | `true` | bold the panel headers and project names |
| `show_key_combos` | `false` | experimental: show each key pressed in the panels bottom-left with the action it ran, for screen shares. Keys typed into a locked pane or a text field never show |
| `show_workspaces` | | show the Workspaces bar |
| `show_diagnostics` | `false` | show the diagnostics card (`Shift+C`) |
| `hide_projects` / `hide_worktrees` / `hide_prs` | `false` | start with that sidebar hidden; the Sessions panel always stays visible |
| `claude_enabled` / `codex_enabled` / `cursor_enabled` | `true` | which harnesses the new-session menu offers |
| `claude_model` / `claude_effort` / `codex_model` / `codex_effort` | | defaults the picker takes on `Enter` |
| `keybindings` | | overrides for any action in [keymap.md](keymap.md), written by Settings → Hotkeys |

### Token prices

The diagnostics card counts Claude tokens exactly, because every turn's numbers are in the
transcript. It shows no dollar figure until you supply the rates, because pacer has none: a
transcript line carries no cost, Claude Code's own stats cache records zero for every model on a
subscription plan, and the model catalog it downloads has no price fields. Rather than guess, the
card leaves the column off.

Fill in `model_prices` by hand in `config.json` and the column appears. Rates are US dollars per
million tokens, taken from Anthropic's pricing page:

```json
"model_prices": {
  "claude-sonnet-4-5": { "input": 3.0, "output": 15.0 },
  "claude-opus-4-1":   { "input": 15.0, "output": 75.0, "cache_read": 1.5 }
}
```

A key matches any model id starting with it, and the longest match wins, so `claude-sonnet-4-5`
prices `claude-sonnet-4-5-20250929` and you only add a dated id to price one snapshot differently.
The three cache rates default to Anthropic's published multiples of `input` (a 5 minute write at
1.25x, a 1 hour write at 2x, a read at 0.1x); `cache_write_5m`, `cache_write_1h` and `cache_read`
override them. A session running a model you have not priced shows `—` rather than a partial total.

On a subscription plan the true per-token bill is zero, so treat the figure as what the same
conversation would have cost through the API.

The settings overlay never writes `model_prices`, so what you type here survives every save. A
reset to defaults does erase it.

## Environment

| Variable | Effect |
|---|---|
| `PACER_EDITOR` | editor command, ahead of the `editor` setting |
| `PACER_LOG` | `RUST_LOG`-style filter for both daemon and TUI (`PACER_LOG=debug`) |
| `PACER_CLOUD_MIRROR_SECS` | cloud-mirror refresh cadence in seconds; `0` turns the follow off |
| `PACER_DATA_DIR` | data dir (database, config, logs) — how tests and parallel instances isolate themselves |
| `PACER_RUNTIME_DIR` | runtime dir holding the socket / endpoint file and pidfile |
| `PACER_METRICS_SAMPLE_MS` | how often the daemon samples CPU, memory and token use; tests shorten it |
| `PACER_AGENT_CMD` | replace every agent CLI with one command line, taken verbatim (`make dev AGENT=/bin/cat`) |
| `PACER_INSTALL_URL` | install-script URL used by `pacer upgrade` and `pacer ssh` |
| `PACER_INSTALL_DIR` | install destination for `install.sh` (default `~/.local/bin`) |

Agent PTYs additionally carry `PACER_AGENT_ID`, `PACER_API_URL` and `PACER_API_TOKEN` so hooks and
`pacer rename` / `worktree` / `spawn` can find their own session. They are scrubbed from plain terminals.
