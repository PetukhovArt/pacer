# Changelog

## Unreleased

- Added a `Bold names` setting under Appearance: off draws the panel headers and project names at normal weight, for terminals whose bold face is a heavier font.
- Added `Ctrl+Shift+R` to repaint the whole screen, and an automatic full repaint when the window regains focus, clearing stale glyphs left after the emulator repaints behind pacer's back.
- Added a `Row timestamps` setting under Appearance: turn it off and the dim `2h ago` label leaves the project, worktree and session rows, which gives the columns back to the names.
- Added the main checkout's gitignored files and symlinks to every new worktree, so `.env`, local settings and linked skills are in place before an agent moves in. Ignored directories such as `node_modules` and `target` are left for you to build.
- Fixed a session that had already answered sweeping yellow for another half hour, because a background subagent cancelled earlier in the turn never reported that it had stopped.
- Fixed `⇧←`/`⇧→`/`⇧↑`/`⇧↓` on the splash screen wrecking the saved layout: with no projects yet there is no body to move a panel across, so instead of swapping the panel with its neighbour the keys tore it out into a minimum-width strip and saved that. They now do nothing until the panels are on screen.
- Fixed `h`/`l` and `←`/`→` walking the fixed Projects/Worktrees/PRs/Sessions order regardless of the layout, which skipped past the panel actually next door. They now follow the panels' places on screen, so a panel moved with `⇧←`/`⇧→`/`⇧↑`/`⇧↓` is reached from where it sits, and `h` steps over the terminal pane to a panel moved past it.
- Fixed a session cancelled with `Esc` keeping its red waiting-on-you dot indefinitely, which carried up to the worktree and project rows. The turn is now closed out from the agent CLI's own idle state within a minute.
- Fixed the Projects panel wearing the open workspace's name as its header while the Workspaces bar is off. The header reads `PROJECTS` either way, and the footer nameplate still names the workspace.
- Changed session rows to drop the trailing CLI badge when every session in the tree runs the same CLI. The badge is back as soon as a second kind appears.
- Windows: Fixed the Hotkeys tab warning about `^⇧`, `^→` and `^⌫` chords, which the console delivers fine. It now warns about `^Esc` (the Start menu) and Windows Terminal's own defaults.

## 0.18.0 (2026-09-03)

- Added native Windows 10 and 11 support: pacer runs there the same way it does on macOS and Linux.
- Added movable panels: `Shift+←/→/↑/↓` moves the focused Projects, Worktrees or Sessions panel through the body like a tiling window manager, so you can swap it with a neighbour, stack it above or below the terminal, or park it on any edge of the screen. Horizontal rules drag like the vertical ones, and the arrangement is remembered.
- Added a PRs panel: the open pull requests are a panel of their own, stacked under Worktrees by default, with its own cursor, filter and `Shift+R` toggle, and movable like the other three.
- Added GitLab alongside GitHub, with merge requests, comments, approvals and diffs in the same interface, self-hosted instances included.
- Added review and pipeline status to OPEN PRS rows, as icons to the left of the number, each in its own column.
- Added an `Open PRs filter` setting: the group lists all open pull requests, only yours, or ones you took part in. The login is asked of the forge inside that checkout, so self-hosted GitLab and GHE answer for themselves, and if it can't be determined the list is hidden rather than silently showing everything. Changing the filter refreshes immediately, and a reply to an already-sent request with the old filter is discarded.
- Added pins (`p`) for any number of workspaces, projects, worktrees and sessions. Pinned rows carry a ★, float to the top of their list and survive a restart.
- Added per-column sorting (`Projects sort`, `Worktrees sort`, `Sessions sort`); `⇧S` cycles the sort of the column the cursor is in, between recent, name and creation order. A config predating the split carries one `list_sort`, which all three columns adopt.
- Added an inline list filter (`Ctrl+F`): a fuzzy query narrows the focused panel, `Enter` parks it, `Esc` clears and closes.
- Added orphaned sessions (`⇧O`), so deleting a worktree no longer takes the conversations with it. pacer saves the agent CLIs' session ids first and lists them per project, with branch, date and transcript size. `Enter` resumes one in whatever worktree the cursor is on, and Claude is told the old directory is gone. The list draws on two sources: pacer's own table (all three CLIs, from this version on) and Claude Code's transcripts on disk, which also turns up sessions lost earlier.
- Added a thread tree for PR conversations: replies sit under the root comment with `├` / `└` branches instead of interleaved by time, and the root shows the file and diff line it hangs on and a `✓ resolved` mark. GitLab threads come from `/discussions`, and "requested changes" shows as a verdict alongside approval. If the endpoint doesn't answer, the previous flat list remains.
- Added an npm install route: `npm install -g @petukhovart/pacer` on macOS, Linux and Windows alike. npm fetches the binary for your platform only, with no build and no download at install time.
- Added prebuilt binaries for five targets (macOS arm64/x64, Linux x64/arm64 on static musl, and Windows x64) to each GitHub release. `install.sh` and `pacer upgrade` take them from there.
- Changed lists to default to `created` order, a stable creation order, so they no longer re-sort themselves as you work. `recent` and `name` are opt-in through `⇧S` or the settings, and cursors stay on the same row across a sort change rather than the same index.
- Changed `n` to go straight to naming the session when only one harness is enabled, instead of offering a menu of one.
- Changed the documentation layout: the README is now the pitch and the install, and the rest moved into `docs/` (getting started, keymap, CLI, configuration, Windows, releasing), plus a guide to reaching the TUI from a phone over Tailscale.
- Windows: Added loopback TCP with a bearer token between the daemon and the TUI, in place of a unix socket. Port and token live in an endpoint file in the per-user runtime dir.
- Windows: Added the daemon guard, so a second daemon refuses to start over a live one; `pacer kill` and the version check work.
- Windows: Added windowless background operation for the daemon, which survives the client closing.
- Windows: Added end-to-end session support: agent launch, output, scrollback and turn status.
- Windows: Added whole-process-tree kill when a session closes, so nothing is left in the background.
- Windows: Added session memory metrics, read through the Win32 APIs.
- Windows: Fixed agent CLIs installed through npm failing to launch.
- Windows: Fixed `~/` not resolving when `HOME` is unset; it now resolves through `USERPROFILE`.
- Windows: Fixed background `git` and `gh` calls flashing console windows.
- Windows: Fixed links not opening in the browser, for `Enter` on a pull-request row and `⌥`-click on a URL.
- Windows: Fixed the editor modal failing when `vim` and `nano` are not on `PATH`; it now finds Git for Windows' own copy.
- Windows: Fixed UTF-8 files opening in the editor as mojibake.
- Windows: Fixed `Shift+Enter` and other Ctrl/Alt chords not reaching console programs.
- Windows: Fixed a multi-line paste (`Ctrl+V`) reaching the agent line by line instead of as one message.
- Windows: Fixed `pacer ssh` mishandling the console, the exit code and `Ctrl+C`.
- Windows: Fixed `pacer upgrade` failing obscurely; it now declines and points at `npm update` or `cargo install`.
