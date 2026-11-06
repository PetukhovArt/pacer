import { createChannel, hasBackend, invoke } from '../bridge';
import type { AiView } from './ai';
import type { ConnectorView } from './api';

export type ConnectorsChanged = { connectors: ConnectorView[]; ai: AiView[] };

const listeners = new Set<(event: ConnectorsChanged) => void>();
let started = false;

function start() {
  if (started || !hasBackend) return;
  started = true;
  const channel = createChannel<ConnectorsChanged>((event) => {
    for (const listener of listeners) listener(event);
  });
  invoke<void>('connectors_watch', { onEvent: channel }).catch(() => {
    started = false;
  });
}

export function onConnectorsChanged(listener: (event: ConnectorsChanged) => void) {
  listeners.add(listener);
  start();
}

function nudgeConnectors() {
  if (hasBackend) void invoke<void>('connectors_reconnect').catch(() => undefined);
}

if (typeof window !== 'undefined') {
  window.addEventListener('online', nudgeConnectors);
}
