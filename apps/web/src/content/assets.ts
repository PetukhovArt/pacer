import { connectorAsset, connectorsList } from '../connectors/api';

const POOL = 4;

let bases: Promise<Map<string, URL>> | null = null;
const assets = new Map<string, Promise<string | null>>();

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

export async function resolveAgainst(connector: string | undefined, raw: string): Promise<string | null> {
  if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(raw)) return raw.startsWith('//') ? `https:${raw}` : raw;
  if (!connector) return null;
  const base = (await connectorBases()).get(connector);
  if (!base) return null;
  try {
    return new URL(raw, base).href;
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

async function swapImage(img: HTMLImageElement, connector: string, isCurrent: () => boolean): Promise<void> {
  const raw = img.getAttribute('src');
  if (!raw || raw.startsWith('data:')) return;
  const absolute = await resolveAgainst(connector, raw);
  if (!absolute || !isCurrent()) return;
  const base = (await connectorBases()).get(connector);
  let host: string;
  try {
    host = new URL(absolute).host;
  } catch {
    return;
  }
  if (!base || host !== base.host) return;
  const data = await fetchAsset(connector, absolute);
  if (!isCurrent()) return;
  img.src = data ?? absolute;
}

export async function loadConnectorImages(root: HTMLElement, connector: string, isCurrent: () => boolean): Promise<void> {
  const images = [...root.querySelectorAll('img')];
  let next = 0;
  const worker = async () => {
    while (next < images.length && isCurrent()) {
      const img = images[next++];
      if (img) await swapImage(img, connector, isCurrent);
    }
  };
  await Promise.all(Array.from({ length: Math.min(POOL, images.length) }, worker));
}
