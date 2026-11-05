import { connectorAsset, connectorsList } from '../connectors/api';
import { fsReadBinary } from '../files/api';

const POOL = 4;
const ABSOLUTE = /^(?:[a-z][a-z\d+.-]*:|\/\/)/i;

let bases: Promise<Map<string, URL>> | null = null;
const assets = new Map<string, Promise<string | null>>();
const localAssets = new Map<string, Promise<string | null>>();

function connectorBases(): Promise<Map<string, URL>> {
  bases ??= connectorsList()
    .then((list) => {
      const map = new Map<string, URL>();
      for (const item of list) {
        try {
          map.set(item.id, new URL(item.base_url));
        } catch {
          continue;
        }
      }
      return map;
    })
    .catch(() => {
      bases = null;
      return new Map<string, URL>();
    });
  return bases;
}

export function isRelative(raw: string): boolean {
  return raw !== '' && !raw.startsWith('#') && !ABSOLUTE.test(raw);
}

export async function resolveAgainst(connector: string | undefined, raw: string, baseUrl?: string): Promise<string | null> {
  if (ABSOLUTE.test(raw)) return raw.startsWith('//') ? `https:${raw}` : raw;
  if (baseUrl) {
    try {
      return new URL(raw, baseUrl).href;
    } catch {
      return null;
    }
  }
  if (!connector) return null;
  const base = (await connectorBases()).get(connector);
  if (!base) return null;
  try {
    return new URL(raw, base).href;
  } catch {
    return null;
  }
}

function toRawUrl(url: string): string {
  try {
    const parsed = new URL(url);
    parsed.pathname = parsed.pathname.replace(/^(\/[^/]+\/[^/]+)\/blob\//, '$1/raw/').replace('/-/blob/', '/-/raw/');
    return parsed.href;
  } catch {
    return url;
  }
}

export function localTarget(basePath: string, raw: string): { path: string; hash: string } | null {
  const dir = basePath.replace(/\\/g, '/').replace(/\/*$/, '/');
  const encoded = dir
    .split('/')
    .map((part, index) => (index === 0 && /^[a-z]:$/i.test(part) ? part : encodeURIComponent(part)))
    .join('/');
  const root = encoded.startsWith('/') ? `file://${encoded}` : `file:///${encoded}`;
  try {
    const url = new URL(raw, root);
    let path = decodeURIComponent(url.pathname);
    if (/^\/[a-z]:/i.test(path)) path = path.slice(1);
    return { path, hash: decodeURIComponent(url.hash.slice(1)) };
  } catch {
    return null;
  }
}

async function fetchAsset(connector: string, url: string): Promise<string | null> {
  const key = `${connector}\n${url}`;
  let pending = assets.get(key);
  if (!pending) {
    pending = connectorAsset(connector, url)
      .then((asset) => `data:${asset.mime};base64,${asset.base64}`)
      .catch(() => {
        assets.delete(key);
        return null;
      });
    assets.set(key, pending);
  }
  return pending;
}

function fetchLocal(path: string): Promise<string | null> {
  let pending = localAssets.get(path);
  if (!pending) {
    pending = fsReadBinary(path)
      .then((file) => `data:${file.mime};base64,${file.base64}`)
      .catch(() => {
        localAssets.delete(path);
        return null;
      });
    localAssets.set(path, pending);
  }
  return pending;
}

async function swapRemote(img: HTMLImageElement, connector: string | undefined, baseUrl: string | undefined, isCurrent: () => boolean): Promise<void> {
  const raw = img.getAttribute('src');
  if (!raw || raw.startsWith('data:')) return;
  const resolved = await resolveAgainst(connector, raw, baseUrl);
  if (!resolved || !isCurrent()) return;
  const absolute = baseUrl && isRelative(raw) ? toRawUrl(resolved) : resolved;
  if (!connector) {
    img.src = absolute;
    return;
  }
  const base = (await connectorBases()).get(connector);
  let host: string;
  try {
    host = new URL(absolute).host;
  } catch {
    return;
  }
  if (!base || host !== base.host) {
    if (baseUrl && isRelative(raw)) img.src = absolute;
    return;
  }
  const data = await fetchAsset(connector, absolute);
  if (!isCurrent()) return;
  img.src = data ?? absolute;
}

async function swapLocal(img: HTMLImageElement, basePath: string, isCurrent: () => boolean): Promise<void> {
  const raw = img.getAttribute('src');
  if (!raw || !isRelative(raw)) return;
  const target = localTarget(basePath, raw);
  if (!target) return;
  const data = await fetchLocal(target.path);
  if (data && isCurrent()) img.src = data;
}

async function pool(images: HTMLImageElement[], isCurrent: () => boolean, swap: (img: HTMLImageElement) => Promise<void>): Promise<void> {
  let next = 0;
  const worker = async () => {
    while (next < images.length && isCurrent()) {
      const img = images[next++];
      if (img) await swap(img);
    }
  };
  await Promise.all(Array.from({ length: Math.min(POOL, images.length) }, worker));
}

export function loadRemoteImages(root: HTMLElement, connector: string | undefined, baseUrl: string | undefined, isCurrent: () => boolean): Promise<void> {
  return pool([...root.querySelectorAll('img')], isCurrent, (img) => swapRemote(img, connector, baseUrl, isCurrent));
}

export function loadLocalImages(root: HTMLElement, basePath: string, isCurrent: () => boolean): Promise<void> {
  return pool([...root.querySelectorAll('img')], isCurrent, (img) => swapLocal(img, basePath, isCurrent));
}
