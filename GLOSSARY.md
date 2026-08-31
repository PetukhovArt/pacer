# Nebula

Terminal multiplexer for AI coding agents: a daemon owns the PTY sessions; a TUI client attaches and detaches without losing work.

## The tree

**Workspace**
A named group of Projects. Exactly one Workspace is open per TUI window; the others keep running in the background.
_Avoid_: project group

**Open Workspace**
The Workspace a given TUI window is scoped to. Two windows can have different Open Workspaces.
_Avoid_: selected workspace, current workspace

**Default Workspace**
The built-in Workspace `default` — present in every install, cannot be deleted.

**Project**
A git repository registered with nebula. Lives inside a Workspace.
_Avoid_: repo

**Worktree**
Where a Session runs: the Root Worktree or a real `git worktree`. Two agents in two Worktrees do not interfere.
_Avoid_: branch (a branch is a property of a Worktree, not the Worktree itself)

**Root Worktree**
The Project's own checkout. Every Project has exactly one.
_Avoid_: main checkout, root row

**Worktree Dir**
The convention for where new Worktrees go: `<repo>/../<repo-name>-worktrees/<branch>`.
_Avoid_: sibling dir

**Session**
One row in the sessions list: an Agent or a Terminal Session, bound to a Worktree and backed by a PTY Session in the Daemon.
_Avoid_: tab

**Agent**
A Session running an agent CLI (`claude`, `codex`, `cursor-agent`).
_Avoid_: the claude

**Agent Kind**
Which CLI an Agent runs: `claude`, `codex`, or `cursor`.
_Avoid_: harness, agent type

**Terminal Session**
A Session that is a plain shell in the Worktree's directory — no agent.
_Avoid_: shell tab

**PR Session**
A Claude Agent created from an open-PR row and constrained to work only on that PR. Starts in the Root Worktree.

**PR Row**
The open pull request on a Worktree's branch. Stores nothing — it comes back from the git poll each time.
_Avoid_: pull request link

**Link**
A URL previously pinned to a Worktree. A legacy of older versions: existing rows stay visible and editable; new ones are not created.

**Pin**
A row (Workspace, Project, Worktree, Agent, terminal tab) the user marked with `p`. Pinned rows wear a ★ and float to the top of their list, whatever the sort mode; any number can be pinned at once. Persisted client-side in the daemon's `ui_state` blob. Distinct from a Link ("a URL pinned to a Worktree"), which is legacy vocabulary.

**Archive**
The state of an Agent whose PTY is released and whose row is moved to the ARCHIVED group. Coming back resumes the conversation.
_Avoid_: deletion (Archive is reversible)

## Processes

**Daemon**
The background process that owns every PTY Session, the store, git operations, and agent statuses. Survives the TUI closing.
_Avoid_: server

**TUI**
The client (`nebula` with no arguments) that attaches to the Daemon. Closing it kills nothing.
_Avoid_: the app, window

**PTY Session**
The Daemon-owned pseudo-terminal of one Session, together with its child process.

**Attach**
Connecting a TUI pane to a Session: the Daemon replays scrollback, then streams live output. Detaching does not kill the process.
_Avoid_: drill in

**Resume**
Restoring an Agent from its stored CLI session id; a fresh session when the id is gone.
_Avoid_: restore, restart (a restart is a new process; Resume continues the conversation)

## Statuses

**Agent Status**
The Daemon-owned state of an Agent: Fresh, Running, Finished, Needs Feedback, Terminated, Disconnected.

**Fresh**
The agent has not run a turn yet.
_Avoid_: gray

**Running**
A turn is in progress.
_Avoid_: yellow, thinking, mid-turn

**Finished**
The turn is complete.
_Avoid_: done, green

**Needs Feedback**
The agent waits on a human: a permission prompt or a question.
_Avoid_: red, waiting on me

**Terminated**
The agent process died mid-run.

**Disconnected**
The Daemon restarted while the agent was live.

**Unseen**
The flag "a turn finished and nobody has looked". Not the same as Finished: this is the read/unread axis.
_Avoid_: unread ≠ done

**Mark Seen**
Clearing Unseen — happens on Attach to the session.

**Rollup**
Parent rows (Worktree, Project, Workspace) show their children's worst status: red beats yellow; violet whenever anything Unseen is underneath.
_Avoid_: parent dot

**Stop Gate**
The rule that a turn does not count as Finished while its subagents are still alive.
_Avoid_: "the session went green too early"

## Agents and hooks

**Managed Hooks**
The hook groups nebula writes into the agent CLI's config at every spawn; user hooks are preserved.
_Avoid_: the hooks nebula installs

**Hook Receiver**
The Daemon's loopback HTTP endpoint that the agent CLIs' hooks report events to. Not MCP.
_Avoid_: hook server

**Auto-Title**
A session left on its default name gets a 3–4 word title from the agent after the first prompt. A name a human typed always wins.
_Avoid_: auto name, self-rename

**Worktree Relocation**
Moving a Session to another Worktree via `nebula worktree`: the row moves at once; the CLI restarts inside the Worktree with the conversation resumed.
_Avoid_: move into a worktree

**Starting Prompt**
The first prompt passed to the CLI as an argument on a cold spawn. Never persisted — Resume does not replay it.
_Avoid_: first prompt

**Agent Preset**
A saved launch definition: name, Agent Kind, model and effort, prefix and postfix around the task.
_Avoid_: agent modal

## Daemon mechanisms

**Prewarm Pool**
Pre-booted agent CLIs that get adopted when an Agent is created, so Attach lands on a booted screen.
_Avoid_: warm spares as "sub agents" (they are not subagents)

**Warm Spare**
One pre-booted CLI from the Prewarm Pool: a real process with no store row.

**Idle Reaper**
The mechanism that kills idle PTYs in Worktrees nobody is viewing. Working agents and ones waiting on a human are spared; a reaped Agent resumes on the next Attach.
_Avoid_: auto suspend

## Cloud sessions

**Cloud Session**
An Agent row whose work runs in Claude's cloud rather than a local PTY.

**Cloud Launch**
Creating a Cloud Session: the task goes to the cloud as a process argument — no place for secrets.

**Cloud Teleport**
A repeatable snapshot pull of the cloud transcript and branch into a local session — a fork, not a live link. Always in a Cloud Worktree.

**Cloud Worktree**
The `cloud-<id>` Worktree a Cloud Session is re-homed into before Attach or Teleport — the cloud CLI switches branches.

**Cloud Mirror**
A periodic re-Teleport so the pane follows the cloud agent. Ends on the first keystroke into the pane.
