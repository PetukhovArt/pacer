# Windows

pacer runs natively on Windows 10 and 11 — the whole workspace builds, lints and tests there, and the
TUI, the daemon and agent sessions work the same way they do on macOS and Linux. This page covers what
is different.

## Install

The short way — no Rust toolchain needed:

```powershell
npm install -g @petukhovart/pacer
```

The command is `pacer`. `npm update -g @petukhovart/pacer` updates it.

Releases also carry a prebuilt `pacer-x86_64-pc-windows-msvc.zip` — unzip it and put `pacer.exe` anywhere
on your `PATH`.

From source:

```powershell
cargo install --git https://github.com/PetukhovArt/pacer pacer --locked
```

That drops `pacer.exe` into `~\.cargo\bin`. From a checkout, `cargo install --path crates/pacer` does the
same.

There is no self-updater on this platform: `install.sh` is a POSIX script, so `pacer upgrade` declines
and points back at the commands above.

**You want:**

- A recent Rust toolchain — <https://rustup.rs>.
- At least one agent CLI on `PATH`: `claude`, `codex` or `cursor-agent`. npm-installed CLIs (`.cmd`
  shims) are resolved and launched correctly.
- Git for Windows. Beyond git itself, it is where pacer finds `vim` for the editor modal when no editor
  is on `PATH` (`<git root>\usr\bin\vim.exe`).
- `gh` and/or `glab`, if you want the pull-request views.

## Which terminal

Any modern console host works — Windows Terminal, WezTerm, Alacritty. The classic `conhost.exe` window
predates a lot of what the TUI draws; prefer Windows Terminal.

## What differs under the hood

- **Transport.** Unix keeps an `AF_UNIX` socket in a `0700` runtime dir, where a successful connect is
  already proof of identity. Windows has no such socket, so the daemon binds a loopback TCP listener and
  a client presents a bearer token — the same model the hook receiver already uses on every platform.
  Port and token live in an endpoint file beside the pidfile, inside the per-user `%TEMP%\pacer`, which
  the profile's ACL already closes to other unprivileged users.
- **Daemon lifetime.** The daemon runs windowless in the background and survives the client closing. A
  second daemon won't start over a live one, and `pacer kill` stops it cleanly.
- **Process trees.** Closing a session kills the agent's whole process tree — nothing is left running in
  the background — and memory metrics are read through the Win32 APIs rather than parsed from `ps`.
- **No flashing windows.** Background `git` / `gh` invocations are spawned with `CREATE_NO_WINDOW`.
- **Paths.** `~/` resolves through `USERPROFILE` when no `HOME` is set, and canonical paths are handed
  out in ordinary `C:\…` form rather than the `\\?\C:\…` verbatim form git refuses.
- **Input.** `Shift+Enter` and other Ctrl/Alt chords reach console programs, and a multi-line paste
  (`Ctrl+V`) arrives at the agent as one message instead of line-by-line.

## Development loop

There is no `make` on Windows, and installing one would not help much: the Makefile's dev and install
targets are unix-only throughout (`shasum`, `/tmp`, `ps`, `kill`, no `.exe` suffix). Call cargo directly
instead:

```powershell
cargo check --workspace --all-targets    # make check — fastest feedback
cargo clippy --workspace --all-targets   # make lint
cargo fmt --all -- --check               # make ci, first step
cargo test --workspace                   # make test
```

**Never run a fresh build against your real daemon.** `make dev` does that isolation with a wrapper;
here you set the two environment variables yourself, and `cargo run` then gets its own daemon, port,
token and database:

```powershell
$env:PACER_RUNTIME_DIR = "$env:TEMP\pacer-dev"
$env:PACER_DATA_DIR    = "$env:USERPROFILE\.pacer-dev\main"
$env:PACER_AGENT_CMD   = 'cmd.exe'   # optional: stub agents, spawn no real claude
cargo run
```

Give each checkout its own pair of paths — two checkouts sharing a runtime dir means the second TUI
silently drives the first one's daemon. `pacer kill` with the same variables set stops that dev daemon;
deleting the two directories resets the instance.

`PACER_AGENT_CMD` is one program name, taken verbatim with no arguments, so the unix `make dev
AGENT=/bin/cat` idiom has no direct translation — name a harmless console program instead.

To install a build for real use and cut the live daemon over to it:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\update.ps1
```

It builds `--release`, moves the live `pacer.exe` aside (Windows locks a running executable, but renaming
it is allowed), copies the fresh build into `~\.cargo\bin`, and kills the daemon so the next launch runs
the new code. **Run it from a terminal outside pacer** — the cutover stops every session. A build failure
stops before anything is touched.

Two e2e suites (PTY and TUI) are marked as requiring a Unix host and are skipped on Windows; the rest of
the suite, including the Windows transport, token and daemon-lock smoke tests, runs.
