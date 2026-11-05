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

async function readConfig(repo: string): Promise<string> {
  try {
    return await fsReadText(joinPath(repo, '.git/config'));
  } catch {
    const pointer = /^gitdir:\s*(.+)$/m.exec(await fsReadText(joinPath(repo, '.git')))?.[1]?.trim();
    if (!pointer) throw new Error('no gitdir');
    const gitdir = joinPath(repo, pointer);
    try {
      return await fsReadText(joinPath(gitdir, 'config'));
    } catch {
      const common = (await fsReadText(joinPath(gitdir, 'commondir'))).trim();
      return fsReadText(joinPath(joinPath(gitdir, common), 'config'));
    }
  }
}

async function lookup(repo: string) {
  let found: Remote | null = null;
  try {
    const url = originUrl(await readConfig(repo));
    found = url ? parseRemote(url) : null;
  } catch {
    found = null;
  }
  remotes.value = new Map([...remotes.value, [repo, found]]);
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
