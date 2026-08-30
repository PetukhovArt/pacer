# The Workspace Builds And Tests On Windows; ConPTY Blocks Every PTY SESSION — 2026-08-30

**Asked:** `/implement C:\Users\PAN\AppData\Local\Temp\handoff-BVoytS.md` — a handoff from a prior
`grilling` session that had settled six decisions (Q1 loopback TCP + token for the DAEMON SOCKET, Q2
`#[cfg(unix)]` the existing e2e grids + a new Windows smoke grid, Q3 leave `hooks/installer.rs` alone,
Q4 measure `ttyd.win32.exe` before writing anything, Q5 two risks split out as their own tasks, Q6 fork
with `#[cfg]` rather than cut, `upgrade.rs` excepted) and laid out phases 0–6. Goal: a nebula on
Windows 10 / `x86_64-pc-windows-msvc` the user can work in daily. Nothing had been written yet.

**Did:** Branch `windows`, 5 commits. Baseline was exactly the 24 errors in 6 files across 2 crates the
handoff predicted; `cargo check --workspace --all-targets`, `cargo build`, `cargo clippy` and
`cargo test --workspace` are now all green here (12 test binaries, 743 tests). Everything is additive
and `#[cfg]`-gated, so no Unix path changed behaviour.

- **DAEMON SOCKET** → new `nebula-core/src/transport.rs`: `Stream` / `Listener` / `Authorizer` with one
  API on both platforms. Windows binds `127.0.0.1:0` and writes port + 32-byte hex token to the
  **ENDPOINT FILE** (`<RUNTIME DIR>/daemon.endpoint`, `paths::endpoint_path`); the token is presented as
  a frame *before* `Hello` and compared with `subtle::ct_eq` — the HOOK RECEIVER's model verbatim, so
  there is one local-authorization mechanism and not two. `server.rs::accept_loop` clones one
  `Authorizer` before the loop and authorizes inside the spawned task, never on the accept loop.
- **PIDFILE LOCK** → `lifecycle.rs::try_lock_exclusive`, `LockFileEx` vs `flock` behind one signature;
  `nebula-tui/src/ipc.rs` mirrors it (it cannot depend on `nebula-daemon`, so `e2e_windows.rs` asserts
  the two offsets agree). **DAEMON SETSID** → `ipc.rs::detach`, `DETACHED_PROCESS |
  CREATE_NEW_PROCESS_GROUP` (not `CREATE_NO_WINDOW` — the point is *no console at all*). SIGTERM/SIGINT
  → `lib.rs::wait_for_termination_signal`; NEBULA KILL's fallback → `TerminateProcess`.
- **Process-tree kill** → extracted from `pty/mod.rs` into new `pty/kill.rs` as `ProcessGroup`
  (`claim` / `leader_alive` / `kill_all`): `killpg` on Unix, a Job Object with
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` on Windows. The watchdog thread now holds its own clone and is
  platform-free. **Unverified** — see the ConPTY gotcha.
- **Launch environment** → extracted from `registry.rs` into new `launch.rs` (`interactive_shell` /
  `wrap_for_user_env` / `program_is_installed`). The `$SHELL -l -i -c` wrap and its `setsid` probe are
  unchanged on Unix; Windows launches the CLI directly with PATH × PATHEXT resolution.
  `program_is_installed` returns `Option<bool>` so `probe_cli` keeps its old "could not tell ⇒ assume
  installed, cache nothing" behaviour.
- **Path audit (phase 3)** → `paths::canonical_or_raw` + `paths::contains` in `nebula-core`; every caller
  in `registry.rs`, `git.rs`, `daemon/lib.rs` and `tui/ipc.rs` goes through the pair. Wider than the
  handoff predicted: not only `try_reparent_agent_by_cwd`'s half-canonicalized compare but *every* path
  that leaves `canonicalize` and reaches git.
- **Metrics** → `metrics.rs::process_table` is the new seam (`snapshot_from_table` untouched); `sysinfo`
  under `cfg(windows)` only. `nebula-core/src/mem.rs` goes native: `GetProcessMemoryInfo`,
  `GlobalMemoryStatusEx`. `env::home_dir` falls back to `USERPROFILE`.
- **NEBULA SSH** → `ssh.rs::hand_terminal_to_ssh`; Windows spawns + waits + `exit(code)` and suppresses
  its own Ctrl+C for the session (`SetConsoleCtrlHandler(NULL, TRUE)`) so `-t` forwards it to the
  remote. The remote sh prelude is untouched — the remote is POSIX either way. **NEBULA UPGRADE** cut,
  not ported (Q6's exception): it wraps a `sh` script fetching assets that do not exist for this target.
  `install_url` / `KILL_HINT` stay, since NEBULA SSH and NEBULA TUNNEL install on the *remote*.
- **Tests** → `e2e_pty.rs` / `e2e_tui.rs` take a one-line `#![cfg(unix)]`, bodies untouched (Q2's
  condition). New `crates/nebula/tests/e2e_windows.rs`, 5 tests, all green: ENDPOINT FILE + HANDSHAKE,
  the token refused, a second DAEMON refused on the PIDFILE LOCK, DETACHED_PROCESS survival, lock-offset
  agreement. Q4 (`ttyd.win32.exe`) not reached — NEBULA BROWSER is downstream of a working PTY.
- **CLAUDE.md** — the LOCAL HARNESS OVERRIDES line saying the workspace does not build here is now
  replaced with what is still true.

**Gotchas:**

- **Windows file locks are mandatory, not advisory.** `LockFileEx` on byte 0 of the pidfile made the
  file unreadable to every other process — `read_to_string` fails with ERROR_LOCK_VIOLATION (os error
  33), silently breaking `kill_by_pidfile`'s pid lookup and `daemon_exe_path`. `cargo check` cannot see
  it; `e2e_windows.rs` caught it on its first run. Lock a byte 1 GiB in, past any content.
- **`portable-pty` 0.9 spawns a ConPTY child that never runs on this machine.** The host's own handshake
  (`ESC[?9001h ESC[?1004h ESC[6n`) reaches the master reader, the child's output never does, and the
  child hangs or exits `0xC0000142` (STATUS_DLL_INIT_FAILED). Reproduced outside nebula with
  `cmd.exe /c echo` as the child, with WezTerm's sideloaded `conpty.dll` on *and* off PATH, and with the
  agent sandbox on *and* off. **Every PTY SESSION is unverifiable here until this is resolved** — the
  Job Object kill, SCROLLBACK RING replay, AGENT ENV, NEBULA BROWSER.
- **A ConPTY child that never runs never exits, so it cannot be reaped** — the reader thread never sees
  EOF and the *test binary hangs at exit* instead of failing. Several `cargo test` runs "hung" for
  10 minutes for this reason and looked like a build problem.
- **Leaked console hosts exhaust the desktop heap, and the symptom is `0xC0000142` on unrelated spawns.**
  Mid-session the same diagnostic flipped between "child started" and STATUS_DLL_INIT_FAILED depending
  on how many orphaned `conhost`/`OpenConsole`/`cmd` processes had piled up. Reap before trusting any
  ConPTY result.
- **`portable-pty` sideloads `conpty.dll` from PATH** (`win/psuedocon.rs::load_conpty` prefers it over
  `kernel32`), so WezTerm's ships the pseudo console here — visible as `OpenConsole.exe` children and a
  different handshake byte sequence than the system conhost's.
- **`cmd.exe` given arguments it does not recognise opens an *interactive* shell.** As a test stub that
  leaks one live child per test. `where.exe` takes the arguments, fails, and exits — that is what a
  "spawns and dies" stub wants.
- **`read_frame` gets `ConnectionReset` on Windows where Unix gets a clean `Ok(None)`** when the peer
  drops the socket with data in flight, so "the daemon hung up on me" has two shapes here.
- `std::env::temp_dir()` is already per-user on Windows, so the RUNTIME DIR needs no uid suffix and no
  explicit ACL — the profile's inherited ACL is the same boundary 0700 draws.
- Windows does not always make a listening port bindable again by the next statement after the listener
  drops; `browser.rs`'s "free again → back to the default" assertion raced the OS rather than the code.
- `nebula add` canonicalizes *before* it connects, so a missing path fails locally and never spawns a
  daemon — worth knowing when writing a test that wants the auto-spawn.
