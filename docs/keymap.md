# Keymap

Defaults — every one of them is rebindable in Settings → Hotkeys (`s`). `Ctrl+q` stays hardwired so you
can't trap yourself.

## Navigate

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab`, `h`/`l` or `←`/`→`, `j`/`k` | move focus / selection through visible panels; the walk stops at both ends (`Tab` at the terminal pane, `Shift+Tab` at the first visible panel) instead of cycling, and landing on a live pane takes its input. `h`/`l` stop one short of each end, and a double tap there (`l`,`l` at Sessions, `h`,`h` at the first visible panel) jumps the boundary; so do `k`,`k` on a panel's first row (up into the workspaces bar) and `j`,`j` in the bar (back down) |
| `Ctrl+→` | cross into the terminal pane *without* taking its input |
| `Enter` | drill in; on a session: attach |
| `/` | fuzzy jump across every workspace, project, worktree, session and open PR — in *every* workspace, each row pathed `workspace/project/branch/session`, so typing another workspace's name jumps you into it (`Ctrl+n`/`p` move, `Ctrl+o` opens the hit, `Ctrl+f` just lands the selection on it) |
| `Ctrl+F` | inline filter: a fuzzy query that narrows the focused panel; `Enter` parks it, `Esc` clears then closes |
| `Shift+S` | cycle the focused column's sort: recent → name → created (pins always float first) |
| `p` | pin / unpin the selected workspace, project, worktree or session — pins float to the top of their list |

## Projects & worktrees

| Key | Action |
|---|---|
| `n` / `d` (Projects) | add project / remove from list |
| `o` (any panel) | add ("open") a project — same prompt as `n`, from any focus |
| Add-project prompt: type + `Tab`, `↓↑` / `→` / `←` | browse for the repo: type to filter (bash-style Tab completion), arrows pick a directory, `→` steps in, `←` steps up, `Enter` adds the highlighted (or typed) path; `●` marks git repos |
| `r` (Projects) | rename the row — a label, not a move: the folder on disk keeps its name and hangs off a `└` under the new one. An empty name puts the row back on the folder's name |
| `n` / `d` (Worktrees, checkout row) | new worktree / delete (typed confirm — deletes files) |
| New-worktree prompt | type a sentence and it is slugified (`fix login redirect` → `fix-login-redirect`); `Enter` on the empty prompt takes a random `<adj>-<noun>-<verb>` |
| `n`, `m` / right-click (Worktrees, OPEN PRS row) | new Claude session scoped to that PR; the context menu also opens the PR or its diff |
| `Shift+G` | open the selected repo's page on its git host — the `origin` remote (`git@github.com:o/r.git`, `ssh://`, `https://`) turned into a browsable URL, credentials stripped |

## Sessions

| Key | Action |
|---|---|
| `n` | new session (agent or shell terminal) |
| `Tab` in the Claude row of the picker | toggle Claude Cloud; Cloud adds a wrapped task prompt (`Shift+Enter` or `Ctrl+J` inserts a line) before launch |
| `m` on a cloud row | **Attach cloud session** re-pulls the transcript now; **Send to cloud session** queues a message on it |
| `r`, `a`, `u`, `d`, `A` | rename, archive, unarchive, delete, toggle archived |
| `e` | agent presets: saved launch definitions (harness, model, effort, optional prefix/postfix). `Enter` asks for a task and starts the agent with prefix + task + postfix as its first prompt; `a` / `e` / `d` create, edit, delete |
| `Shift+O` | orphaned sessions: conversations whose worktree was deleted, per project. `Enter` resumes one where the cursor is |
| `Shift+D` | delete every row of the focused panel (the confirm lists the casualties) |
| `t` | new shell terminal in the selected worktree's directory (Projects panel: the repo root) |
| `Enter` on an OPEN PRS row | open it in the browser. Resting on the pull request reads it in the pane; `g` shows its diff, `PgUp`/`PgDn` scroll |

## Views

| Key | Action |
|---|---|
| `g` | git diff for the selected worktree — or, on an open-PR row, that pull request's diff: filter, `↑↓` files, `Shift+↑↓` / `PgUp`/`PgDn` / `Ctrl+d`/`u` scroll, `Ctrl+r` marks a file reviewed ✓ |
| `f` / `F` / `b` | find file / find in files (`git grep`) / file tree browser, all scoped to the selected worktree — `Enter` opens the file in an editor modal (at the matched line, for `F`); in `f` and `b`, `Ctrl+y` copies the path |
| `z` | full-screen terminal: collapse the sidebars and lock input into the attached session |
| `Shift+M` | memory usage: RAM per agent/terminal process tree, pacer itself, and the machine-wide share; `↑`/`↓` + `Enter` opens the selected session |

## Workspaces & panels

| Key | Action |
|---|---|
| `w`, or click the `◇ workspace` nameplate bottom-left | workspace switcher: `Enter` opens, `n`/`r`/`d` create/rename/delete. Delete asks first and lands on the tab to the right (or left, from the last tab). Per window — switching here leaves your other pacer instances where they are |
| `Shift+W` | show / hide the Workspaces bar across the top: one tab per workspace with the rolled-up status of the agents under it, plus a count of the ones that finished unread |
| `1`–`9` (or `⌘1`–`⌘9`) | open that numbered tab without leaving the panel you're in. `⌘` is what the tabs advertise, but Terminal.app and most other emulators never encode it into pty bytes — the bare digit is the one that always arrives |
| In the Workspaces bar: `←`/`→`, `↓`/`Enter`, `n`/`r`/`d`, `m` | the cursor is the open workspace, so `←`/`→` switches; `↓` or `Enter` steps down into the first visible panel; create / rename / delete the open one (delete refuses a non-empty workspace); `m` or right-click lists the same verbs |
| `Shift+P` / `Shift+B` | show / hide the Projects / Worktrees panel. The terminal pane takes the released width; showing a panel restores its remembered width without stealing focus. Persisted as `hide_projects` / `hide_worktrees` |

## General

| Key | Action |
|---|---|
| `Shift+H` | ssh hosts: every `pacer ssh` / `pacer tunnel` destination, newest first. `Enter`/click reconnects (quits this TUI and execs a fresh `pacer ssh` — local sessions keep running), `a` types a new `user@host [dir]`, `d` removes |
| `m` or right-click | context menu for whatever's selected |
| `s` | settings overlay (theme, editor, which agents to offer and their defaults, timeouts, sorts). Its Hotkeys tab rebinds every key in this table; `R` inside it resets everything to the defaults. A first open lands on the tab strip; reopening within a minute of closing lands back on the tab and row you left |
| `Shift+N` | replay the startup splash (any key returns) |
| `?` | help overlay |
| `q` / `Ctrl+c` | quit the TUI (sessions keep running) |

## Terminal pane

| Key | Action |
|---|---|
| anything | forwarded raw to the PTY |
| `Ctrl+q` | back to panels (also expands sidebars) — `Ctrl+]`, `Ctrl+Esc` and `Ctrl+←` do the same, for terminals that eat one of them |
| mouse wheel | scrollback (arrow keys on alt-screen apps) |

## Typed fields

Every prompt, filter and query is the same line editor: `←`/`→` and `⌥←`/`⌥→` move by character / word,
`Ctrl+a` / `Ctrl+e` jump to the ends, `⌥⌫` deletes a word, `Ctrl+u` / `Ctrl+k` kill the line.

## Mouse

Left-click selects/attaches, right-click opens context menus, double-click in the terminal selects a
word, `⌥`-click opens the URL or `file:line` under the cursor (browser / editor modal), and dragging a
visible panel border resizes it. Hidden panels keep their last width for the next time they are shown. A
click outside any modal (help, settings, a confirm, a prompt, the pickers) dismisses it, exactly as `Esc`
would. Text selection: hold `Shift` while dragging (mouse-capture bypass — same as tmux).
