# Platform vector: decisions against Herdr

Not a spec. A record of what was settled while comparing pacer with Herdr, what was rejected and
why, and which branches are still open. Half the tree was never asked, so nothing here is ready to
implement except where it points at the mermaid spec beside it.

## The competitive read

Herdr (`herdrdev/herdr`, Rust, pre-1.0, roughly 35k stars, one full-time developer) is a tmux-shaped
multiplexer with an agent-status sidebar and a socket API. Its shape is panes, tabs and workspaces.

Where it is ahead of pacer:

- 14 or more agent CLIs supported. pacer has three, and a fourth needs Rust in the daemon.
- Status without hooks: periodic screen matching against TOML rules, so an unknown CLI still reports
  something. Hooks exist as an optional overlay that supersedes the screen rules.
- A public socket API plus CLI: agents spawn panes, prompt each other, wait until another agent is
  blocked, subscribe to events, export and import layouts.
- Plugins with a marketplace. Third parties shipped a token-metrics sidebar widget and a Telegram
  bridge without the maintainer writing either.
- Real multiplexing: splits, tabs, zoom, mouse, prefix keys.
- Distribution: brew, mise, an install script per platform, documentation in three languages.

Where pacer is ahead:

- A domain tree, Workspace to Project to Worktree to Session, with status rolled up through it.
  Herdr has no concept of a project or a repository.
- Pull requests in the same interface, GitHub and GitLab, with review and pipeline status, thread
  trees, diffs, and an agent scoped to a PR.
- Reading code without leaving: diff viewer with reviewed-file bookkeeping, fuzzy file find, git
  grep, file tree with preview.
- Status precision. Hooks plus OSC 9;4 drive a state machine with six states, against Herdr's four,
  and Claude under Herdr is screen-matched only.
- Browser and phone access through a tunnel, not just ssh. Herdr cannot use Windows as a remote
  target host.
- Agent presets, cloud sessions, orphaned sessions, pins, per-column sort, inline filter.
- MIT, against Apache 2.0 with AGPL in the earlier reviews.

## Settled

1. **The vector is platform, not parity.** Grow as a runtime other code is written against, rather
   than as a second tmux. Herdr's audience came from its API, not from its sidebar, and chasing
   splits and tabs is a fight on its ground.
2. **The public API is a separate layer from the TUI protocol.** JSON over HTTP on loopback with a
   bearer token, events as a stream, built on the axum receiver the hooks already use. The internal
   MessagePack protocol stays private and keeps breaking freely. Two mappings is the price for being
   able to change the TUI without breaking third parties.
3. **The CLI is the API's first client, not separate code.** Otherwise the two paths diverge and the
   API ships with no consumer.
4. **Universality comes before plugins.** A new CLI needing daemon code is the gate on the front
   door: someone running Copilot CLI or OpenCode cannot use pacer at all today.
5. **Status gets two layers, hooks over screen matching.** Hooks stay authoritative where they
   exist; screen rules are the fallback that makes an unknown CLI work on the day it ships.
   Recommended, not yet confirmed.
6. **Mermaid preview is a plain feature, not the first plugin.** An extension point designed against
   one consumer is the wrong shape, and a plugin that draws inside our ratatui frame is the most
   expensive kind to commit to. See the mermaid spec beside this file.

## Rejected, with the reason

- **Sandboxing.** Neither tool has it and no incident calls for it. Building guard machinery against
  a violation that has not happened is what the global memory principle warns about.
- **Restore after a daemon restart.** Already at parity: the boot sweep marks live agents
  disconnected (`store.rs:773`, verified) and the next attach respawns with the CLI's own resume flag
  (`registry.rs:2729`, verified). The earlier claim that this was a gap was wrong.
- **Splits, tabs and prefix keys.** Of the whole tmux surface the one real gap is two sessions on
  screen at once. The rest is Herdr's ground.
- **Opening the internal protocol as the public one.** It is at version 36 and bumps with most
  features; every bump would break third-party code.

## Open

- **API v1 scope.** The recommendation was a core of list and inspect, create a session, send input,
  read output, wait on a status, subscribe to events, create and delete a worktree, plus
  `report_status` and `set_metadata`. No focus or layout operations, because UI operations change
  most often and would destabilise the contract. Not confirmed.
- **What `read` returns.** Output today is raw VT bytes, so no client can read a screen without its
  own terminal emulator. The recommendation is for the daemon to return screen text, since it
  already owns a vt100 parser. Not confirmed.
- **Plugin format.** Manifest plus command plus declared capabilities, following what Herdr does,
  versus something else. Never asked.
- **Distribution.** brew and mise alongside the current npm, curl and cargo routes. Never asked.
- **Who writes the screen-matching rules** for a new CLI, and how they are shipped and updated when
  a vendor changes its output. Never asked.
- **API versioning and stability policy.** Never asked.

## What already exists, so it is not new work

Traced during the session, all verified:

- The tree, session creation, input, output, status and worktree operations all exist as request
  variants in `protocol.rs`, so the API layer is mostly a mapping, not new behaviour.
- The hooks HTTP receiver with a bearer token is in `hooks/mod.rs:190` and works the same on every
  platform, unlike the TUI transport.
- `PROTOCOL_VERSION` is 36 at `protocol.rs:11`.

Genuinely missing for the API core: a blocking wait on status, screen text as opposed to raw bytes, a
prompt-and-submit operation distinct from raw input, `report_status` from an external source,
`set_metadata` with a TTL, the public HTTP surface itself, and CLI wrappers over all of it.
