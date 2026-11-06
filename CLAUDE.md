# Berth

Terminal multiplexer for AI coding agents: a daemon owns the PTYs and outlives the TUI that started
it; a ratatui TUI attaches over the daemon socket. That socket is an `AF_UNIX` socket on unix and a
loopback TCP listener plus a bearer token on Windows — one or the other, never layered, and hidden
behind one API in `crates/berth-core/src/transport.rs`. Code is the source of truth: read
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
- `event_loop.rs`, `ui.rs` and `registry.rs` grew huge by accretion. Adding to one of the three: put
  the new code in a new module beside it and call it from there, rather than growing the file.

<important if="you are writing text a user reads: the changelog, the README, docs, release notes or a PR body">

- `CHANGELOG.md` is for users, not for us. A change gets an entry under `## Unreleased` only if
  someone who runs `berth` would notice it — a feature, a fix, a changed key, a new install route.
  Refactors, tests, CI, docs, tooling and anything under `scripts/` get no entry.
- `CHANGELOG.md` follows the Claude Code changelog shape: `## <version>` and nothing below it, one
  flat bullet per change on a single line, each starting with `Added` / `Fixed` / `Changed` /
  `Removed`, sorted by that verb. No `###` sections, no bold lead-ins, no em dashes. Platform-specific
  entries carry a `Windows:` prefix before the verb. Backticks for keys, settings and commands.
- Every text a user reads (`README.md`, `CHANGELOG.md`, `docs/`, release notes, PR bodies) is
  written without em dashes, bold lead-ins, forced groups of three, or sales language. The
  `humanizer` skill applies these rules when it is installed; the rules hold either way.

</important>

## Dev workflow

**Never run freshly built code against your real daemon.** A dev instance needs its own runtime dir
and its own database, or an unfinished binary is driving your live sessions.

- **unix** — the Makefile is the whole workflow: `make dev` for the isolated instance, `make check`
  for fast feedback, `make ci` for the gate, `make help` for the rest.
- **Windows** — there is no `make`, and its dev targets are unix-only anyway. Call cargo directly,
  set the isolation env vars yourself, and use `scripts\update.ps1` to install and cut over. The
  commands are in `docs/windows.md`.

Work on `main` only: no feature branches, no worktrees.

## Desktop app (apps/, packages/, docker/)

The Tauri desktop app, its Vue UI and the ui packages came over from Veyra; `apps/desktop/src-tauri/src/legacy.rs`
holds every old name it still reads (`~/.veyra`, the `veyra` keychain service, `VEYRA_*`, the old Tauri
identifier). Rules there differ from the Rust crates:

- No code comments and no tests in `apps/` and `packages/`. The crates under `crates/` keep their own tests.
- Check a change with `node scripts/checks.mjs <files>` (comments, raw colors, oxlint, knip, types, rustfmt,
  clippy for `berth-desktop`) and `node ~/.claude/hooks/diff-verify.mjs`. Never run a full `vue-tsc` or `tsc`
  over the project. The husky pre-commit runs `checks.mjs` on staged files under `apps/`, `packages/` and
  `docker/` only.
- Colors come from `packages/ui-tokens` through `var(--vr-…)`; the web side reaches Rust only through
  `apps/web/src/bridge.ts`.
- Dev run on Windows, isolated from your real daemon and your real `~/.berth` and keychain:
  `powershell -ExecutionPolicy Bypass -File scripts\dev.ps1 -Desktop` (builds berth, starts a dev daemon on
  this checkout's `BERTH_RUNTIME_DIR`/`BERTH_DATA_DIR`, sets `BERTH_HOME` and `BERTH_SECRETS=file`, runs
  `pnpm dev:desktop` with WebView2 debugging on port 9334, stops the daemon after). Without the script, set the
  same variables and run `berth daemon` and `pnpm dev:desktop` yourself. `apps/desktop/scripts/dev.mjs` runs
  `tauri dev` with `tauri.dev.conf.json` (identifier `dev.berth.desktop.dev`, title `Berth (dev)`) and
  defaults each of those variables to a dev location when it is unset.
- The user's own desktop app is a working copy under `%LOCALAPPDATA%\Berth` (`scripts/install-app.ps1`,
  `apps/desktop/src-tauri/src/working_copy.rs`), built from committed `main` and updated only from its
  `Update Berth…` menu. Never launch, rebuild or kill it from a task; test the updater with `BERTH_APP_ROOT`
  and the other overrides listed in `docs/windows.md`.

<important if="you are changing paths, environment variables or hook installation">

## On-disk names

Paths, env vars and the hook tag are load-bearing on data that already exists on user machines:
`crates/berth-core/src/paths.rs` (runtime dir, socket, pidfile, endpoint file, data dir, database),
`crates/berth-core/src/env.rs` (env vars) and `crates/berth-daemon/src/hooks/installer.rs` (the hook
tag and the Cursor rule file). Changing any of them needs a migration, not a find-and-replace — the
tests pass either way, and existing installations lose their sessions, pins and settings.

The pacer → berth rename is the precedent: every old name sits in `crates/berth-core/src/legacy.rs`,
and `crates/berth-daemon/src/pacer_migration.rs` copies pacer's data dir into an absent berth one on
first start, never touching the old one. Keep it until nobody runs pacer 0.20 any more. The
Makefile's `dev-seed` target and `docs/configuration.md` quote the same paths, so keep the three in
step.

</important>

## Docs

`README.md` is the pitch: why, install, quickstart, key features. Detail lives in `docs/` —
`getting-started.md`, `keymap.md`, `cli.md`, `configuration.md`, `windows.md`, `development.md`,
`releasing.md`, `remote-access-tailscale.md`. Put new user-facing writing in the right one instead
of growing the README.

## Plans

`.claude/plans/` is the backlog. Read the ones covering your area before designing anything — the
design may already be settled, with the reason each alternative lost. `<date>-<name>.prd.md` is a
spec, ready to implement; `<date>-<name>.decisions.md` records what was settled, what was rejected
and what nobody has answered yet, so don't build from one without asking. Delete a plan once it
ships, or say at the top that it didn't.

<important if="you are cutting a release, publishing a package or changing repository settings">

## Publishing

Releasing is automatic and described in `docs/releasing.md`: a `version` bump in the root
`Cargo.toml` landing on `main` builds the five targets, tags the commit, publishes the GitHub release
and publishes to npm — one workflow, no button. The deliberate act is therefore the version bump, so
never bump `version` unless releasing is what was asked for; every other change leaves it alone.

Repository visibility stays a human decision: never change it on your own.

</important>
