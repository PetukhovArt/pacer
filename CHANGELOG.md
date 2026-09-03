# Changelog

Notable changes, newest first. Entries tagged **(Windows)** are platform-specific; everything else
applies everywhere.

## Unreleased

### Windows support

pacer runs natively on Windows 10 and 11, the same way it does on macOS and Linux.

- **(Windows)** The daemon and the TUI talk over loopback TCP with a bearer token instead of a unix
  socket. Port and token live in an endpoint file in the per-user runtime dir.
- **(Windows)** A second daemon refuses to start over a live one; `pacer kill` and the version check
  work.
- **(Windows)** The daemon runs windowless in the background and survives the client closing.
- **(Windows)** Sessions work end to end: agent launch, output, scrollback, turn status.
- **(Windows)** Closing a session kills the agent's whole process tree — nothing is left in the
  background.
- **(Windows)** Agent CLIs installed through npm launch correctly.
- **(Windows)** Session memory metrics are read through the Win32 APIs.
- **(Windows)** `~/` resolves through `USERPROFILE` when `HOME` is unset.
- **(Windows)** Background `git` / `gh` calls no longer flash console windows.
- **(Windows)** Links — `Enter` on a pull-request row, `⌥`-click on a URL — open in the browser.
- **(Windows)** The editor modal launches even when `vim` / `nano` are not on `PATH`, by finding Git
  for Windows' own copy.
- **(Windows)** UTF-8 files open in the editor readable, without mojibake.
- **(Windows)** `Shift+Enter` and other Ctrl/Alt chords reach console programs.
- **(Windows)** A multi-line paste (`Ctrl+V`) reaches the agent as one message, not line by line.
- **(Windows)** `pacer ssh` hands over the console, the exit code and `Ctrl+C` correctly.
- **(Windows)** `pacer upgrade` declines and points at `cargo install` instead of failing obscurely.

### Added

- **Movable panels.** `Shift+←/→/↑/↓` moves the focused Projects / Worktrees / Sessions panel through
  the body like a tiling window manager: swap it with a neighbour, stack it above or below the
  terminal, or park it on any edge of the screen. Horizontal rules drag like the vertical ones. The
  arrangement is remembered.
- **PRs panel.** The open pull requests are a panel of their own, stacked under Worktrees by default,
  with its own cursor, filter and `Shift+R` toggle — and movable like the other three.
- **GitLab alongside GitHub.** Merge requests, comments, approvals and diffs in the same interface,
  self-hosted instances included.
- **Review and pipeline status on OPEN PRS rows**, as icons to the left of the number, each in its own
  column.
- **`Open PRs filter` setting** — the group lists all open pull requests, only yours, or ones you took
  part in. The login is asked of the forge inside that checkout, so self-hosted GitLab/GHE answer for
  themselves; if it can't be determined the list is hidden rather than silently showing everything.
  Changing the filter refreshes immediately, and a reply to an already-sent request with the old filter
  is discarded.
- **Pins (`p`)** — any number of workspaces, projects, worktrees and sessions. Pinned rows carry a ★,
  float to the top of their list and survive a restart.
- **Per-column sorting** — `Projects sort`, `Worktrees sort`, `Sessions sort`; `⇧S` cycles the sort of
  the column the cursor is in (recent, name, or creation order). A config predating the split carries
  one `list_sort`, which all three columns adopt.
- **Inline list filter (`Ctrl+F`)** — a fuzzy query narrows the focused panel; `Enter` parks it, `Esc`
  clears and closes.
- **Orphaned sessions (`⇧O`)** — deleting a worktree no longer takes the conversations with it. pacer
  saves the agent CLIs' session ids first and lists them per project, with branch, date and transcript
  size. `Enter` resumes one in whatever worktree the cursor is on, and Claude is told the old directory
  is gone. The list draws on two sources: pacer's own table (all three CLIs, from this version on) and
  Claude Code's transcripts on disk, which also turns up sessions lost earlier.
- **PR conversations as a thread tree** — replies sit under the root comment with `├` / `└` branches
  instead of interleaved by time; the root shows the file and diff line it hangs on and a `✓ resolved`
  mark. GitLab threads come from `/discussions`, and "requested changes" shows as a verdict alongside
  approval; if the endpoint doesn't answer, the previous flat list remains.

### Changed

- **Lists default to `created` order** — a stable creation order, so they no longer re-sort themselves
  as you work. `recent` / `name` are opt-in through `⇧S` or the settings. Cursors stay on the same row
  across a sort change rather than the same index.
- **A lone enabled harness skips the picker** — `n` goes straight to naming the session instead of
  offering a menu of one.

### Install and documentation

- **Install from npm** — `npm install -g @petukhovart/pacer` on macOS, Linux and Windows alike. npm
  fetches the binary for your platform only; no build, no download at install time.
- **Prebuilt binaries for five targets** — macOS arm64/x64, Linux x64/arm64 (static musl) and Windows
  x64, attached to each GitHub release. `install.sh` and `pacer upgrade` take them from there.
- **Documentation** — the README is now the pitch and the install; the rest moved into `docs/`
  (getting started, keymap, CLI, configuration, Windows, releasing), plus a guide to reaching the TUI
  from a phone over Tailscale.
