# pacer

**Mission control for your coding agents.**

Run **Claude Code**, **Codex** and **Cursor** across every project and git worktree you own — from one
terminal, one keyboard, one tree. A background daemon owns the PTYs, so they keep working when you close
the UI.

```sh
npm install -g @petukhovart/pacer
pacer add ~/code/my-app
pacer
```

This package is a thin launcher: the actual binary is a small Rust executable that npm installs for your
platform only (macOS arm64/x64, Linux x64/arm64, Windows x64).

**Prerequisite:** at least one agent CLI on your `PATH` — `claude`, `codex` or `cursor-agent`. pacer
spawns them; it doesn't ship them.

Documentation, screenshots and the full keymap: <https://github.com/PetukhovArt/pacer>

MIT licensed.
