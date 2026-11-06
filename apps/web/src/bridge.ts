import { Channel as TauriChannel, invoke as tauriInvoke } from '@tauri-apps/api/core';

export const isDesktop = '__TAURI_INTERNALS__' in window;
const isRemote = !isDesktop && document.querySelector('meta[name="berth-remote"]') !== null;
export const hasBackend = isDesktop || isRemote;

export type BridgeChannel<T> = { onmessage: (message: T) => void };

type Frame = { ready?: true; id?: number; ok?: unknown; err?: string; channel?: number; message?: unknown };
type Pending = { resolve: (value: unknown) => void; reject: (reason: unknown) => void };

const RECONNECT_DELAY_MS = 2000;
const WS_PATH = '/__berth/ws';

class RemoteChannel<T> implements BridgeChannel<T> {
  constructor(
    readonly id: number,
    public onmessage: (message: T) => void,
  ) {}

  toJSON() {
    return { __channel: this.id };
  }
}

const channels = new Map<number, RemoteChannel<unknown>>();
const pending = new Map<number, Pending>();
let nextId = 1;
let socket: Promise<WebSocket> | null = null;

function receive(frame: Frame, onReady: () => void) {
  if (frame.ready) return onReady();
  if (typeof frame.channel === 'number') {
    channels.get(frame.channel)?.onmessage(frame.message);
    return;
  }
  if (typeof frame.id !== 'number') return;
  const call = pending.get(frame.id);
  if (!call) return;
  pending.delete(frame.id);
  if (frame.err !== undefined) call.reject(frame.err);
  else call.resolve(frame.ok ?? null);
}

function connect(): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(`${scheme}://${location.host}${WS_PATH}`);
    let opened = false;
    ws.onmessage = (event: MessageEvent<string>) => {
      receive(JSON.parse(event.data) as Frame, () => {
        opened = true;
        resolve(ws);
      });
    };
    ws.onclose = () => {
      socket = null;
      for (const call of pending.values()) call.reject('remote connection closed');
      pending.clear();
      if (opened) setTimeout(() => location.reload(), RECONNECT_DELAY_MS);
      else reject(new Error('remote connection refused'));
    };
  });
}

async function remoteInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  socket ??= connect();
  const ws = await socket;
  const id = nextId++;
  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
    ws.send(JSON.stringify({ id, cmd, args: args ?? {} }));
  });
}

export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return isDesktop ? tauriInvoke<T>(cmd, args) : remoteInvoke<T>(cmd, args);
}

export function createChannel<T>(onMessage: (message: T) => void = () => undefined): BridgeChannel<T> {
  if (isDesktop) return new TauriChannel<T>(onMessage);
  const channel = new RemoteChannel<T>(nextId++, onMessage);
  channels.set(channel.id, channel as RemoteChannel<unknown>);
  return channel;
}

export function closeChannel<T>(channel: BridgeChannel<T>) {
  channel.onmessage = () => undefined;
  if (channel instanceof RemoteChannel) channels.delete(channel.id);
}

export async function openUrl(url: string): Promise<void> {
  if (isDesktop) {
    try {
      await tauriInvoke<void>('plugin:opener|open_url', { url });
      return;
    } catch {
      window.open(url, '_blank', 'noopener');
      return;
    }
  }
  window.open(url, '_blank', 'noopener');
}
