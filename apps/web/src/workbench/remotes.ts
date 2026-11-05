import { shallowRef } from 'vue';
import { fsReadText } from '../files/api';

export type Remote = { host: string; path: string };

export const remotes = shallowRef<ReadonlyMap<string, Remote | null>>(new Map());
const started = new Set<string>();

function joinPath(base: string, rel: string): string {
  if (/^([a-zA-Z]:)?[\\/]/.test(rel)) return rel;
  return `${base.replace(/[\\/]+$/, '')}/${rel}`;
}

function originUrl(config: string): string | null {
  let inOrigin = false;
  for (const raw of config.split(/\r?\n/)) {
    const line = raw.trim();
    if (line.startsWith('[')) {
      inOrigin = /^\[\s*remote\s+"origin"\s*\]$/.test(line);
      continue;
    }
    const url = inOrigin ? /^url\s*=\s*(.+)$/.exec(line)?.[1] : undefined;
    if (url) return url.trim();
  }
  return null;
}

function parseRemote(remote: string): Remote | null {
  const text = remote.trim();
  let host: string;
  let path: string;
  if (text.includes('://')) {
    try {
      const url = new URL(text);
      host = url.hostname;
      path = decodeURIComponent(url.pathname);
    } catch {
      return null;
    }
  } else {
    const colon = text.indexOf(':');
    if (colon < 0) return null;
    const left = text.slice(0, colon);
    host = left.slice(left.lastIndexOf('@') + 1);
    if (host.length < 2) return null;
    path = text.slice(colon + 1);
  }
  path = path.replace(/^\/+|\/+$/g, '').replace(/\.git$/, '').replace(/\/+$/, '');
  if (!host || !path) return null;
  return { host: host.toLowerCase(), path };
}

async function readConfig(repo: string): Promise<{ text: string; main: boolean }> {
  try {
    return { text: await fsReadText(joinPath(repo, '.git/config')), main: true };
  } catch {
    const pointer = /^gitdir:\s*(.+)$/m.exec(await fsReadText(joinPath(repo, '.git')))?.[1]?.trim();
    if (!pointer) throw new Error('no gitdir');
    const gitdir = joinPath(repo, pointer);
    try {
      return { text: await fsReadText(joinPath(gitdir, 'config')), main: false };
    } catch {
      const common = (await fsReadText(joinPath(gitdir, 'commondir'))).trim();
      return { text: await fsReadText(joinPath(joinPath(gitdir, common), 'config')), main: false };
    }
  }
}

export async function inspectClone(repo: string): Promise<{ remote: Remote | null; main: boolean }> {
  try {
    const { text, main } = await readConfig(repo);
    const url = originUrl(text);
    return { remote: url ? parseRemote(url) : null, main };
  } catch {
    return { remote: null, main: false };
  }
}

async function lookup(repo: string) {
  const { remote } = await inspectClone(repo);
  remotes.value = new Map([...remotes.value, [repo, remote]]);
}

export function lookupRemotes(repos: readonly string[]): void {
  for (const repo of repos) {
    if (!repo || started.has(repo)) continue;
    started.add(repo);
    void lookup(repo);
  }
}

export function connectorBase(baseUrl: string): Remote | null {
  try {
    const url = new URL(baseUrl);
    return { host: url.hostname.toLowerCase(), path: url.pathname.replace(/^\/+|\/+$/g, '') };
  } catch {
    return null;
  }
}
