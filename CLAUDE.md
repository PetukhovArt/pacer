# Nebula

Terminal multiplexer for AI coding agents: a daemon owns the PTYs; a ratatui TUI attaches over a
socket. Code is the source of truth — read `ARCHITECTURE.md` for the map and `GLOSSARY.md` for the
domain vocabulary before touching IPC, hooks, or the daemon.

- Dev workflow lives in the Makefile (`make help`). Fast feedback: `make check`; full gate: `make ci`.
- Never run freshly built code against your real daemon — `make dev` boots an isolated instance
  (own daemon, socket, DB, per checkout); `make dev AGENT=/bin/cat` stubs agent spawns.
- `vendor/vt100` is a patched fork — don't bump or replace it.
- Split what you touch: `event_loop.rs`, `ui.rs`, `registry.rs` grew huge by accretion; extract the
  piece you edit instead of adding to the pile.
