import { connectorAsset } from '../connectors/api';
import { escapeHtml } from '../content/highlight';

export async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

export function readStored<T>(key: string): T[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? '[]');
    return Array.isArray(value) ? (value as T[]) : [];
  } catch {
    return [];
  }
}

export function writeStored<T>(key: string, value: T[]): void {
  localStorage.setItem(key, JSON.stringify(value));
}

export type RecentPage = { connector: string; id: string; title: string; space: string };

const RECENT_KEY = 'veyra.confluence.recent';

export function recentPages(connector: string): RecentPage[] {
  return readStored<RecentPage>(RECENT_KEY).filter((page) => page.connector === connector);
}

export function rememberPage(page: RecentPage): void {
  const rest = readStored<RecentPage>(RECENT_KEY).filter((item) => !(item.connector === page.connector && item.id === page.id));
  writeStored(RECENT_KEY, [page, ...rest].slice(0, 30));
}

const MARK_OPEN = '@@@hl@@@';
const MARK_CLOSE = '@@@endhl@@@';
const HIGHLIGHT_TAGS = new Set(['MARK', 'B', 'STRONG', 'EM']);

function safeNodes(nodes: NodeListOf<ChildNode>): string {
  let out = '';
  nodes.forEach((node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      out += escapeHtml(node.textContent ?? '');
    } else if (node instanceof HTMLElement) {
      const inner = safeNodes(node.childNodes);
      const highlighted = HIGHLIGHT_TAGS.has(node.tagName) || node.classList.contains('search-highlight');
      out += highlighted ? `<mark>${inner}</mark>` : inner;
    }
  });
  return out;
}

export function highlightHtml(excerpt: string): string {
  const source = excerpt.split(MARK_OPEN).join('<mark>').split(MARK_CLOSE).join('</mark>');
  const doc = new DOMParser().parseFromString(source, 'text/html');
  return safeNodes(doc.body.childNodes);
}

const avatars = new Map<string, Promise<string | null>>();

export function avatarSrc(connector: string, url: string): Promise<string | null> {
  const key = `${connector}\n${url}`;
  let cached = avatars.get(key);
  if (!cached) {
    cached = connectorAsset(connector, url)
      .then((asset) => `data:${asset.mime};base64,${asset.base64}`)
      .catch(() => null);
    avatars.set(key, cached);
  }
  return cached;
}
