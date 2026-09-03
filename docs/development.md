# Development

Everything below is the unix workflow. **On Windows there is no `make`** — the equivalents are in
[windows.md](windows.md#development-loop).

```sh
make help                 # every target
make check                # typecheck — fastest feedback
make ci                   # the whole gate: fmt check, clippy, tests
cargo build --release     # → target/release/pacer (~4 MB)
cargo test                # unit + end-to-end suite (spawns real daemons/PTYs)
```

Never run freshly built code against your real daemon. `make dev` boots an isolated instance — its own
daemon, socket and database, one per checkout — and `make dev AGENT=/bin/cat` stubs agent spawns.
`make dev-seed` copies your real projects and settings into it, `make dev-reset` wipes it, `make dev-ls`
lists every checkout's instance. On Windows, see [windows.md](windows.md#development-loop).

## Layout

| Crate | What it owns |
|---|---|
| `pacer-core` | shared protocol, entities, paths, transport |
| `pacer-daemon` | PTYs, SQLite, hook receiver, status engine |
| `pacer-tui` | the ratatui client |
| `pacer` | the binary and its subcommands |

`vendor/vt100` is a patched copy of the terminal parser, wired in through `[patch.crates-io]`: rows
scrolled out of a top-anchored scroll region go to scrollback instead of being discarded, so wheel-up
over a codex session has something to show. Don't bump or replace it.

[ARCHITECTURE.md](../ARCHITECTURE.md) is the map of how the pieces fit; [GLOSSARY.md](../GLOSSARY.md) is
the domain vocabulary. Read both before touching IPC, hooks or the daemon.

## Releases

Bump `version` in the root `Cargo.toml` and land it on `main`. CI builds macOS (arm/intel), Linux
(x64/arm64, static musl) and Windows (x64) binaries, tags the commit, attaches them to a GitHub release
— which is what `install.sh` downloads — and publishes the same binaries to npm. The version bump is
the release, so leave `version` alone in every other change.

The whole runbook, including the npm packaging and what it needs set up once, is in
[releasing.md](releasing.md).
