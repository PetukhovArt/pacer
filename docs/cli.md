# Command reference

## Everyday

```sh
berth                    # launch the TUI (auto-starts the daemon)
berth --workspace <name> # launch it on a named workspace; each instance keeps its own, so
                         # two windows can sit on two workspaces at once
berth add <dir>          # add a repo as a project, named after its root directory
berth add .              # same, for the repo you're in (bare `berth <dir>` / `berth .` also work)
berth upgrade            # install the latest release (--force on a dev build)
```

## Daemon

```sh
berth daemon             # run the daemon (normally auto-spawned)
berth daemon --foreground  # daemon with logs to stderr, for debugging
berth kill               # stop the daemon and all sessions cleanly
```

Upgrading while a daemon is running is safe: sessions keep running on the old binary until you
`berth kill` and relaunch.

## Run by agents

These exist so a session can act on its own tree; you rarely type them yourself.

```sh
berth rename <title>     # title the current session (--force to retitle a named one)
berth worktree [name] [--base <ref>]
                         # move the current session into a worktree of its project, creating the
                         # branch if it's new. No name invents one; --base picks the start point
berth spawn <task> [--kind <claude|codex|cursor>]
                         # start a new agent session beside the current one, in the same worktree,
                         # opening on <task>; --kind defaults to this session's harness
```

## Workspaces

```sh
berth workspace add <name>     # create a workspace (a named project group)
berth workspace open <name>    # open it in the next instance you launch
berth workspace list           # list workspaces; * marks the one new instances open into
berth workspace rename <a> <b> # rename a workspace
berth workspace delete <name>  # delete an empty workspace
```

## Remote access

```sh
berth ssh <host> [dir]   # open berth on a remote machine over ssh (installs it there if missing);
                         # destinations are remembered for the TUI's Shift+H picker
```

```sh
berth tunnel <host> [dir] [--port N] [--remote-port N]
```

That host's berth in a browser tab here, over one ssh tunnel: installs berth there if missing, runs
`berth browser` on its loopback, forwards the port, and opens the local URL. Nothing is exposed on the
remote's network — the tunnel is the only way in — so it needs no `--credential`. If that host already
has a `berth browser` on the port, the tunnel reuses it instead of failing on the clash (a
`--credential` one will ask for it in the tab). Needs ttyd on the remote; `Ctrl+C` takes both ends down.
`--port` is the local end, `--remote-port` the far end when something there already holds that number.

```sh
berth browser [--port N] [--bind ADDR | --public] [--credential USER:PASSWORD] [--no-open]
```

Serve this TUI in a browser tab via ttyd and open it; needs ttyd on `PATH`. With no `--port` it takes
7681 when that's free and a free port otherwise, saying which — so one per checkout can serve at once.
`--port 0` always picks a free one; `--port N` is that port or an error, which is what you want behind an
ssh tunnel.

It listens on `127.0.0.1` unless `--bind` names an interface address or `--public` takes them all
(`0.0.0.0`) — for a berth on a remote box, where the access control is the firewall or security group in
front of the port. That serves a live, writable terminal, so put something in front of it and use
`--credential` to add ttyd's HTTP basic auth on top. `--no-open` serves without launching a desktop
browser, for a box that has none.

For reaching this from a phone over any network, see [remote-access-tailscale.md](remote-access-tailscale.md).
