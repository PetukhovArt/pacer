# Windows

berth runs natively on Windows 10 and 11 — the whole workspace builds, lints and tests there, and the
TUI, the daemon and agent sessions work the same way they do on macOS and Linux. This page covers what
is different.

## Install

The short way — no Rust toolchain needed:

```powershell
npm install -g @petukhovart/berth
```

The command is `berth`. `npm update -g @petukhovart/berth` updates it.

Releases also carry a prebuilt `berth-x86_64-pc-windows-msvc.zip` — unzip it and put `berth.exe` anywhere
on your `PATH`.

From source:

```powershell
cargo install --git https://github.com/PetukhovArt/pacer berth --locked
```

That drops `berth.exe` into `~\.cargo\bin`. From a checkout, `cargo install --path crates/berth` does the
same.

There is no self-updater on this platform: `install.sh` is a POSIX script, so `berth upgrade` declines
and points back at the commands above.

**You want:**

- A recent Rust toolchain — <https://rustup.rs>.
- At least one agent CLI on `PATH`: `claude`, `codex` or `cursor-agent`. npm-installed CLIs (`.cmd`
  shims) are resolved and launched correctly.
- Git for Windows. Beyond git itself, it is where berth finds `vim` for the editor modal when no editor
  is on `PATH` (`<git root>\usr\bin\vim.exe`).
- `gh` and/or `glab`, if you want the pull-request views.

## Which terminal

Any modern console host works — Windows Terminal, WezTerm, Alacritty. The classic `conhost.exe` window
predates a lot of what the TUI draws; prefer Windows Terminal.

## What differs under the hood

- **Transport.** Unix keeps an `AF_UNIX` socket in a `0700` runtime dir, where a successful connect is
  already proof of identity. Windows has no such socket, so the daemon binds a loopback TCP listener and
  a client presents a bearer token — the same model the hook receiver already uses on every platform.
  Port and token live in an endpoint file beside the pidfile, inside the per-user `%TEMP%\berth`, which
  the profile's ACL already closes to other unprivileged users.
- **Daemon lifetime.** The daemon runs windowless in the background and survives the client closing. A
  second daemon won't start over a live one, and `berth kill` stops it cleanly.
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

**Never run a fresh build against your real daemon.** `scripts\dev.ps1` is the Windows counterpart
of `make dev`: it builds, keys a runtime dir and a database to this checkout, stops the dev daemon
left over from the previous run, and starts the TUI on the fresh binary.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\dev.ps1
powershell -ExecutionPolicy Bypass -File scripts\dev.ps1 -Agent cmd.exe   # stub agents out
powershell -ExecutionPolicy Bypass -File scripts\dev.ps1 -Reset           # wipe the dev database
```

Run it in a window of its own, outside berth, and keep your real berth in another. The loop is
edit, quit the TUI with `q`, run the script again. Stopping the old daemon is the load-bearing part:
it detaches and outlives its TUI, so a rebuild that only relaunches the TUI leaves daemon-side
changes running the previous code.

The script sets what you would otherwise export by hand:

```powershell
$env:BERTH_RUNTIME_DIR = "$env:TEMP\berth-dev"
$env:BERTH_DATA_DIR    = "$env:USERPROFILE\.berth-dev\main"
$env:BERTH_AGENT_CMD   = 'cmd.exe'   # optional: stub agents, spawn no real claude
cargo run
```

Give each checkout its own pair of paths — two checkouts sharing a runtime dir means the second TUI
silently drives the first one's daemon. `berth kill` with the same variables set stops that dev daemon;
deleting the two directories resets the instance.

`BERTH_AGENT_CMD` is one program name, taken verbatim with no arguments, so the unix `make dev
AGENT=/bin/cat` idiom has no direct translation — name a harmless console program instead.

To install a build for real use and cut the live daemon over to it:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\update.ps1
```

It builds `--release`, moves the live `berth.exe` aside (Windows locks a running executable, but renaming
it is allowed), copies the fresh build into `~\.cargo\bin`, and kills the daemon so the next launch runs
the new code. **Run it from a terminal outside berth** — the cutover stops every session. A build failure
stops before anything is touched.

## Desktop working copy

The desktop app you use all day should not be the one agents are rebuilding. Install a working copy: a
release build of the committed `main` under `%LOCALAPPDATA%\Berth`, launched from the Start Menu like any
other app, and updated only when you ask.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install-app.ps1 -Launch
```

The script reads `main` from the checkout it sits in, never the files in the working tree, and builds it
in `%LOCALAPPDATA%\Berth\build` (`src` for the exported tree, `target` for an incremental cargo cache,
`logs` for one log per build). The result goes to `app\<build-id>`, `app\current` points at it, `app\previous`
at the build before, and the Start Menu gets a `Berth` shortcut to `app\current\berth-desktop.exe`. The
working copy uses `~\.berth`, the OS keychain and your real daemon.

Inside the app, `Update Berth…` (tray menu and command palette) does the same thing in the background: it
shows the commit it builds, the current step and the last log lines, and `Cancel build` stops it. A failed
build changes nothing and shows its log path. A finished one offers `Restart now` or `Later`; sessions live
in the daemon, so a restart keeps them. `Roll back to previous build` swaps `current` and `previous`.
`app.sourceRepo` in settings points the update at another checkout.

The update never touches the daemon. When the new build speaks a different protocol than the running
daemon, the dialog says so and offers `Update daemon`: it installs that build's `berth.exe` into
`~\.cargo\bin` the way `scripts\update.ps1` does and restarts the daemon, which ends every live terminal.
`install-app.ps1 -UpdateDaemon` and `-Rollback` do the same from a terminal.

Every location can be moved for a test: `BERTH_APP_ROOT` (install root), `BERTH_START_MENU` (shortcut
folder), `BERTH_BIN_DIR` (where `-UpdateDaemon` puts `berth.exe`), `BERTH_HOME`,
`BERTH_RUNTIME_DIR`, `BERTH_DATA_DIR`, and `WEBVIEW2_USER_DATA_FOLDER` for the webview profile.

The dev run (`scripts\dev.ps1 -Desktop` or `pnpm dev:desktop`) stays apart from it: the Tauri identifier
`dev.berth.desktop.dev`, a window titled `Berth (dev)`, `BERTH_HOME` under `~\.berth-dev`, file secrets,
a dev daemon and WebView2 debugging on port 9334.

Two e2e suites (PTY and TUI) are marked as requiring a Unix host and are skipped on Windows; the rest of
the suite, including the Windows transport, token and daemon-lock smoke tests, runs.
