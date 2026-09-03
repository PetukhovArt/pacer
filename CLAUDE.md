# Pacer

Terminal multiplexer for AI coding agents: a daemon owns the PTYs and outlives the TUI that started
it; a ratatui TUI attaches over the daemon socket. That socket is an `AF_UNIX` socket on unix and a
loopback TCP listener plus a bearer token on Windows — one or the other, never layered, and hidden
behind one API in `crates/pacer-core/src/transport.rs`. Code is the source of truth: read
`ARCHITECTURE.md` for the map and `GLOSSARY.md` for the vocabulary before touching IPC, hooks or the
daemon.

`GOTCHAS.md` is the standing traps — the platform quirks, required orderings and settled decisions
this project has already paid for, grouped by area. Read the sections your task touches before you
start, and add a line when a task turns up a trap that will outlive it. In Claude Code the matching
lines arrive on their own: `.claude/hooks/gotchas.mjs` is a `UserPromptSubmit` hook that scores the
file against the prompt and injects what fits. It is best-effort, not a substitute for reading the
file — nothing arrives for a prompt that names nothing it recognizes.

## Invariants

- `vendor/vt100` is a patched fork — don't bump or replace it.
- `CHANGELOG.md` is for users, not for us. A change gets an entry under `## Unreleased` only if
  someone who runs `pacer` would notice it — a feature, a fix, a changed key, a new install route.
  Refactors, tests, CI, docs, tooling and anything under `scripts/` get no entry.
- `event_loop.rs`, `ui.rs` and `registry.rs` grew huge by accretion. Adding to one of the three: put
  the new code in a new module beside it and call it from there, rather than growing the file.

## Dev workflow

**Never run freshly built code against your real daemon.** A dev instance needs its own runtime dir
and its own database, or an unfinished binary is driving your live sessions.

- **unix** — the Makefile is the whole workflow: `make dev` for the isolated instance, `make check`
  for fast feedback, `make ci` for the gate, `make help` for the rest.
- **Windows** — there is no `make`, and its dev targets are unix-only anyway. Call cargo directly,
  set the isolation env vars yourself, and use `scripts\update.ps1` to install and cut over. The
  commands are in `docs/windows.md`.

<important if="you are changing paths, environment variables or hook installation">

## On-disk names

Paths, env vars and the hook tag are load-bearing on data that already exists on user machines:
`crates/pacer-core/src/paths.rs` (runtime dir, socket, pidfile, endpoint file, data dir, database),
`crates/pacer-core/src/env.rs` (env vars) and `crates/pacer-daemon/src/hooks/installer.rs` (the hook
tag and the Cursor rule file). Changing any of them needs a migration, not a find-and-replace — the
tests pass either way, and existing installations lose their sessions, pins and settings.

A rename has been done once, and its shape is the one to copy: `paths::adopt_legacy` reads the old
data dir when the new one does not exist yet — adoption in place, nothing copied, so there is no
half-migrated state to recover from — the Cursor rule installer deletes the file under the old name
before writing the new one, and the runtime dir needs nothing because it holds no state. The
Makefile's `dev-seed` target and `docs/configuration.md` quote the same paths, so keep the three in
step.

</important>

## Docs

`README.md` is the pitch: why, install, quickstart, key features. Detail lives in `docs/` —
`getting-started.md`, `keymap.md`, `cli.md`, `configuration.md`, `windows.md`, `development.md`,
`releasing.md`, `remote-access-tailscale.md`. Put new user-facing writing in the right one instead
of growing the README.

<important if="you are cutting a release, publishing a package or changing repository settings">

## Publishing

Releases and npm publishing are described in `docs/releasing.md`. Both are triggered by a human: never
push a tag, publish a package or change repository visibility on your own.

</important>
