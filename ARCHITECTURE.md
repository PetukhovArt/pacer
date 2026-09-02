# How Pacer works

Pacer is a tmux-style terminal multiplexer for AI coding agents: run Claude / Codex / Cursor CLI
sessions across git repos and worktrees; they keep running after you close the UI. Vocabulary lives
in `GLOSSARY.md`. Details live in the code — this file is the map, not the spec.

## Process model

Two processes, same binary:

1. **Daemon** (`pacer daemon`) — owns every PTY, SQLite, git worktrees, agent status. Spawned by the
   TUI in its own session (`setsid`) when nothing is listening, so it outlives clients and holds no
   controlling terminal.
2. **TUI** (`pacer`) — a ratatui client on the daemon socket: an `AF_UNIX` socket in the `0700` runtime
   dir on unix, or — Windows has none — a loopback TCP listener plus a bearer token there instead.
   One or the other, never both; `pacer-core/src/transport.rs` hides the difference behind one API.
   Quit the TUI and nothing dies; relaunch and scrollback is replayed.

IPC is length-prefixed MessagePack: `ClientRequest` in (CRUD, attach, keystrokes, resize),
`ServerEvent` out (entity deltas, status, PTY output). Attach replays the per-PTY ring buffer, then
streams live output.

## Domain tree

**Workspace → Project → Worktree → Session** (an agent or a plain terminal). Exactly one workspace is
open per TUI instance. Worktrees are real `git worktree`s under `<repo>/../<repo-name>-worktrees/`.
Everything persists in SQLite under the data dir.

## Agent status (not MCP)

At spawn the daemon writes managed hooks into the agent CLI's config; the hooks curl the daemon's
loopback HTTP receiver with a per-boot bearer token. Hook events plus the PTY's OSC 9;4 progress
escapes drive a per-agent state machine that maps to the colored status dots. Claude and Codex share
one hooks dialect; Cursor speaks its own. The same hook channel powers auto-titling.

## Side paths

Each has a subcommand and a module named after it — start from the CLI entry in `crates/pacer`:
`pacer ssh` (remote hosts), `pacer tunnel` / `pacer browser` (the TUI in a browser tab via ttyd),
cloud sessions (`claude --cloud` / `--teleport`), agent presets, the prewarm pool, the idle reaper,
and the memory modal's metrics sweep.

## Crate layout

| Crate | Role |
|---|---|
| `pacer` | Thin CLI: no args → TUI; `daemon`, `kill`, `rename`, `upgrade`, `ssh`, `browser`, … |
| `pacer-core` | Shared protocol, entities, IDs, paths, codec |
| `pacer-daemon` | PTYs, SQLite, git, hook receiver, status engine |
| `pacer-tui` | ratatui UI, keyboard/mouse, attach/scrollback |

`vendor/vt100` is a patched fork (DECSTBM scrollback fix — see its Cargo.toml); don't swap it for
crates.io vt100.
