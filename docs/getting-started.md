# Getting started

A walkthrough of the TUI, from an empty list to a tree of running agents. Key defaults are in
[keymap.md](keymap.md); every one of them is rebindable in Settings → Hotkeys (`s`).

## 1. Add a repo

pacer is project-first, and a project is just a git checkout:

```sh
pacer add ~/code/my-app       # or, from inside the repo: pacer add .
```

## 2. Open the TUI

A bare `pacer` launches it and auto-starts the daemon:

```sh
pacer
```

Up to four columns, left to right: **Projects → Worktrees → Sessions → Terminal**. `Tab` / `Shift+Tab`
(or `h` / `l`, or `←` / `→`) move focus between columns, `j` / `k` move the selection inside one, and
`Enter` drills in. The walk stops at both ends — `Tab` at the terminal pane, `Shift+Tab` at the first
visible panel — instead of cycling. Landing on a live pane hands it the keyboard, so `Tab` all the way
right and start typing at the agent.

`Shift+P` shows or hides the Projects panel and `Shift+B` does the same for Worktrees. Each toggle is
independent, the terminal pane takes the released width, and showing a panel restores its remembered
size without moving focus.

With no projects yet you get the splash instead — press `n` to add one without leaving the TUI.

## 3. Choose where the agent runs

Select your project, then a worktree. Every project starts with one: the checkout itself. Press `n` in
the Worktrees column to branch off into a real `git worktree` (created under
`<repo>/../<repo-name>-worktrees/<branch>`). That's the point of the column — two agents in two
worktrees edit two directories and never collide. The branch name you type is slugified
(`fix login redirect` → `fix-login-redirect`); an empty prompt takes a random `<adj>-<noun>-<verb>`.

Or skip the column and just ask: tell a Claude session "do this in a worktree" and it creates one
through pacer and moves itself into it.

## 4. Start the agent

With a worktree selected, press `n` in the Sessions column. A menu asks what to run — **Claude**,
**Codex** or **Cursor**; a CLI you never use can be switched off on the settings overlay's Agents tab
and drops out of the menu entirely (with exactly one enabled, `n` skips the menu and goes straight to
naming). `→` on Claude or Codex drills into model and reasoning-effort submenus; `Enter` anywhere takes
your configured defaults. A plain shell is `t`.

Name the session or accept the default, and pacer spawns the CLI in that worktree and drops you
straight into it.

### Agent presets

If you keep starting the same kind of session with the same framing, save it as an **agent preset**:
`e` in the Sessions column lists them, `a` opens a small form — name, harness, model, effort, and an
optional prefix and postfix — and `e` / `d` edit or delete. `Enter` on a preset asks for the task in a
wrapped editor, then launches the CLI with `prefix + task + postfix` as its very first prompt, so the
agent is already working when the pane opens. The row it creates is an ordinary session: it names
itself on that first turn, resumes, and shows status like any other. Presets live in
`agent_presets.json` beside `config.json`.

### Claude Cloud sessions

On the Claude row, `Tab` toggles Cloud mode: after the optional name, enter the task in the wrapped
editor (`Shift+Enter` or `Ctrl+J` adds a line) and pacer launches `claude --cloud <task>`. On accounts
without Claude's live-attach rollout the CLI prints the session URL and exits — so pacer reads the
session id off that output and re-enters the session for you, without being asked.

The row becomes a **mirror** of the cloud session: pacer runs `claude --cloud <id>`, falling back to
`claude --teleport <id>` (the transcript and branch pulled into a local session) when the account can't
attach, and then re-teleports every 45s so turns the cloud agent takes keep landing in the pane. The
badge reads `cloud ↻` while it is following. Since either CLI switches the checkout to the cloud
branch, a row still in the main checkout is first re-homed into a `cloud-<id>` worktree of its own.

A teleport is a snapshot, not a live link, which is why the mirror re-pulls — and why **the first key
you type into the pane ends it**: from then on the session is yours, an ordinary local Claude that
started from a cloud transcript, and pacer stops respawning it under you. `NEBULA_CLOUD_MIRROR_SECS`
changes the cadence; `0` turns the follow off, leaving **Attach cloud session** (the row's `m` menu) as
the manual refresh. To steer the cloud agent without a browser, pick **Send to cloud session** — the
same wrapped editor — and pacer runs `claude -p <message> --cloud <id>` and pulls the transcript
straight after. The reply shows up on a later refresh; the CLI never returns one.

> Cloud tasks are process arguments — don't put secrets in a `claude --cloud` task description.

## 5. Leave — it keeps running

`Ctrl+q` gets you out of the terminal and back to the panels. That's the key to remember: the agent
doesn't care that you stopped watching. Press `q` to quit pacer entirely and the daemon still owns every
PTY — come back with `pacer` an hour later and each session is where you left it, scrollback replayed.

Idle PTYs nobody is watching are reaped after `session_idle_timeout` (5m default) and resume on the next
attach.

## 6. Read the dots instead of the screens

Once you're running more than one agent you stop reading terminals and start reading the Sessions
column.

| Dot | Meaning |
|---|---|
| ● gray | fresh — agent never run |
| ● yellow | running — turn in progress (Stop is gated on active subagents) |
| ● violet | done, unread — turn complete and nobody has looked at it yet |
| ● green | done, read — same finished turn, once the cursor has been on the session |
| ● red | needs feedback — permission prompt or question waiting on you |
| ● magenta | terminated — process died mid-run |
| ○ | disconnected — daemon restarted while the agent was live |

Worktree and project rows roll up their children: red beats yellow beats done, and a parent's dot is
violet whenever anything unread finished under it — so the violet walks up the tree and turns green as
you read your way down it.

A dot going violet while you were looking elsewhere is easy to miss, so pacer counts those for you: when
a running (or red) session finishes a turn that isn't in the pane on screen, its worktree and project
rows grow a violet `n done` badge — the number of terminals you have left to go read — and the session
row itself says `done` where its harness name normally sits. Walking the cursor onto a session previews
it, which reads it: the badges count down as you go and disappear at zero. The flag lives in the daemon,
so it survives closing the TUI and is shared by every client; a turn that finishes in the pane you're
already looking at never counts.

Status comes from agent-CLI hooks, not MCP: pacer merges managed hooks into each agent's config and they
report to the daemon's loopback endpoint. Codex hooks live in `~/.codex/hooks.json` — approve them once
at codex's "Hooks need review" prompt.

## 7. Let them name themselves

Leave a new session on its default name and the agent retitles it after your first prompt — `Fix Login
Redirect` rather than `agent-3`. Type a name yourself (or `r` to rename) and pacer never touches it.

## Pull requests in the tree

pacer reads open pull requests from the forge behind `origin` — **GitHub** through `gh`, **GitLab**
(including self-hosted) through `glab` — and shows them in two places:

- **Under the project**, in the Worktrees column: an `OPEN PRS` group listing every pull request still
  open on the repo, drafts included and badged as such. Fetched when you open the project, re-asked
  every 15 seconds, and again whenever a panel or the terminal window takes focus (one list call per
  project, so a repo with a hundred open PRs still costs one API call). That beat is also how rows
  retire: merge or close a pull request and it stops coming back.
- **Under a worktree**, in the Sessions column: the pull request open on that branch, with a count of
  comments that landed while you were away.

Each row carries review and pipeline status as icons to the left of its number. Rest the cursor on one
and the right-hand pane reads it to you — description, stats and the whole conversation — without
leaving pacer. Threads are rendered as a tree: replies sit under the root comment with `├` / `└`
branches, the root shows the file and line of the diff it hangs on, and resolved threads are marked
`✓ resolved`. "Requested changes" shows as a verdict alongside approval.

`g` opens the pull request's diff in the same viewer your worktree diffs use, `Enter` (or a
double-click) opens it in the browser, and `/` finds it by title. Press `n` — or choose **New Claude
session** from `m` / right-click — to start a Claude session in the project's root worktree with an
injected system prompt that limits all work to that PR and includes its URL. The URL is kept with the
agent, so resume reapplies the same scope.

The **Open PRs filter** setting picks what the group lists: all open pull requests, only yours, or ones
you took part in. The login is asked of the forge inside that checkout, so self-hosted instances answer
for themselves; if it can't be determined the list is hidden rather than silently showing everything.

## Working a worktree

The panels aren't the only view. With a worktree selected, from any panel:

| Key | View |
|---|---|
| **`g`** | **Git diff.** Changed files down the left, the diff on the right, with a live fuzzy filter. On an open-PR row it shows that pull request's diff instead. `Ctrl+r` marks a file reviewed ✓ and sinks it to the bottom — pacer-side bookkeeping only, no git state is touched — and every mark clears itself when HEAD moves or the file changes again, so what's left unticked is genuinely what you haven't read. |
| **`f`** | **Find file.** Fuzzy finder over the worktree. `Enter` opens the file in an editor modal (vim by default; the `editor` setting or `NEBULA_EDITOR` picks another), `Ctrl+y` copies the path — ready to paste into an agent. |
| **`F`** | **Find in files.** `git grep` into the same modal; `Enter` opens the hit at its line. |
| **`b`** | **File tree browser.** Tree on the left, syntax-highlighted preview on the right, and an always-live filter that narrows the tree to matching files and the directories holding them. |

## Keeping long lists usable

- **`/` fuzzy jump** across every workspace, project, worktree, session and open pull request — not just
  the open workspace, so picking a hit somewhere else switches you there on the way.
- **`Ctrl+F` inline filter** narrows the focused panel as you type; `Enter` parks the query, `Esc` clears
  and closes.
- **`p` pins** any number of workspaces, projects, worktrees and sessions. Pinned rows carry a ★, float
  to the top of their list, and survive a restart.
- **`Shift+S` sorts** the column the cursor is in — recent → name → created — and each column remembers
  its own choice. The default is `created`, a stable creation order, so lists don't re-sort themselves
  under you; cursors stay on the same row across a sort change.
- **`w` switches workspace** when one project list gets long (each pacer instance keeps its own — run
  two, on two workspaces, side by side), and `Shift+W` shows or hides the Workspaces bar across the top,
  where every workspace is a tab carrying the rolled-up status of the agents under it.

## Orphaned sessions

Deleting a worktree no longer takes the conversations with it: before removing it, pacer saves the agent
CLIs' session ids. `Shift+O` lists them by project, with branch, date and transcript size. `Enter`
resumes the conversation in whatever worktree the cursor is on, and Claude is told the old directory is
gone. The list is built from two sources — pacer's own table (all three CLIs, from this version on) and
Claude Code's transcripts on disk, which also turns up sessions lost before that.

## Elsewhere

- `t` opens a shell in the selected worktree, `Shift+G` opens the repo's page on its git host.
- `z` full-screens the terminal, `Shift+M` shows RAM per session, `s` opens settings, `?` lists every
  key, and `m` (or right-click) opens a context menu for whatever's selected.
