export const remote = {
  category: 'Remote access',
  def: {
    enabled: {
      label: 'Remote access',
      description: 'Serve Veyra to a phone or browser over the network. Anyone with the pairing link controls your terminals.',
    },
    bind: {
      label: 'Listen on',
      description: 'Tailscale listens only on the 100.x address of this machine, LAN on every network interface, localhost only on this machine.',
      localhost: 'This machine only (localhost)',
      lan: 'Local network (LAN)',
      tailscale: 'Tailscale',
    },
    port: {
      label: 'Port',
      description: 'TCP port of the remote access server.',
    },
  },
  panel: {
    warningTitle: 'Remote access gives full terminal control',
    warning: 'A paired device can type into every session, run commands and read files in your worktrees. Traffic is plain HTTP: prefer Tailscale, which encrypts it, and never expose the port to the internet.',
    off: 'Remote access is off. Turn it on below to get a pairing link.',
    starting: 'Starting the server…',
    running: 'Server is running',
    addresses: 'Addresses',
    pairing: 'Pairing link',
    pairingHint: 'Scan the QR code with the phone camera or open the link in a browser. The device stays paired until the token is regenerated.',
    copy: 'Copy link',
    copied: 'Copied',
    regenerate: 'Regenerate token',
    regenerateHint: 'Creates a new pairing link and disconnects every paired device.',
    clients: 'Connected devices',
    noClients: 'No devices connected',
    connectedAt: 'since {time}',
    qrLabel: 'QR code of the pairing link',
    desktopOnly: 'Remote access is managed in the Veyra app on the computer. On a veyra-server (Docker) host the pairing link and QR code are in the server log: docker compose -f docker/compose.yaml logs veyra.',
    problem: {
      tailscale_missing: 'Tailscale address (100.x) not found. Start Tailscale or choose another network.',
      bind_failed: 'Could not listen on {message}',
      token_store: 'Could not read the pairing token from the system keychain: {message}',
    },
  },
} as const;
