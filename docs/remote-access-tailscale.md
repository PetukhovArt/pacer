# Remote access from a phone via Tailscale

How to open the nebula TUI on a phone from any network, using `nebula browser`
over a Tailscale tailnet. The daemon and its sessions stay on the PC; the phone
gets the TUI in a browser tab through ttyd (HTTP/WebSocket + xterm.js).

Why Tailscale and not a raw `--public` bind: what ttyd serves is a live,
writable terminal, ttyd ships no TLS, and basic auth over plain HTTP sends the
password in the clear. Inside a tailnet the transport is WireGuard-encrypted
and the port is invisible to anyone outside your devices, so none of that is
exposed.

## One-time setup

### On the PC (the machine running the nebula daemon)

1. Install ttyd — `nebula browser` needs it on PATH:
   - Windows: `winget install tsl0922.ttyd`
   - macOS: `brew install ttyd`
   - Debian/Ubuntu: `sudo apt install ttyd`
2. Install Tailscale and log in:
   - Windows: `winget install tailscale.tailscale`, then `tailscale login`
     (opens a browser page to authenticate).
   - Other platforms: see https://tailscale.com/download
3. Note the machine's tailnet address:

   ```
   tailscale ip -4
   ```

   It's a stable `100.x.y.z` address — it survives reboots and network changes.

### On the phone

1. Install the Tailscale app (App Store / Google Play).
2. Log in with the same account and flip the VPN toggle on.

## Connecting

On the PC, in its own terminal (the command blocks while serving; Ctrl+C stops
it):

```
nebula browser --bind <tailscale-ip> --credential USER:PASSWORD --no-open
```

- `--bind <tailscale-ip>` listens only on the tailnet interface — nothing is
  reachable from the LAN or the internet.
- `--credential` adds HTTP basic auth on top; keep it even inside the tailnet.
- `--no-open` skips launching a local browser tab.

On the phone, with Tailscale connected, open:

```
http://<tailscale-ip>:7681
```

and enter the credentials. Sessions live in the daemon, so the phone sees the
same session list as the desktop TUI.

## Troubleshooting

- **Page doesn't load on the phone** — check the Tailscale toggle is on on the
  phone, and that `tailscale status` on the PC shows both devices. On Windows,
  allow ttyd through the firewall if a prompt appears on first run.
- **`nebula browser` says ttyd is missing** — it's not on PATH; open a fresh
  terminal after installing, or reinstall (see setup above).
- **Port 7681 is busy** — `nebula browser` steps to a free port on its own and
  prints which one; use that in the phone URL, or pin one with `--port N`.

## Limits

- The PC must be awake: sessions live in the daemon's PTYs, so sleep or
  shutdown kills them — and there is nothing to attach to until it's back up.
  Disable sleep in the power settings if you rely on this.
- `nebula browser` must be running on the PC for the phone to connect. For
  reaching a machine you can SSH into instead, `nebula tunnel HOST` does this
  whole dance over a single SSH port-forward — see README.
