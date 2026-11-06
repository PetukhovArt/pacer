import { fsListMarkdown } from '../../files/api';

export type MarkdownEntry = { path: string; rel: string; name: string };

export const FILE_PANEL_PREFIX = 'file:';

export function normalizePath(path: string): string {
  return path.replace(/\\/g, '/');
}

export function filePanelId(path: string): string {
  return `${FILE_PANEL_PREFIX}${normalizePath(path)}`;
}

export function isFilePanel(id: string): boolean {
  return id.startsWith(FILE_PANEL_PREFIX);
}

export function fileNameOf(path: string): string {
  return normalizePath(path).split('/').pop() ?? path;
}

function rank(rel: string): number {
  const name = rel.toLowerCase();
  if (/^readme(\.[a-z]+)?\.(md|markdown|mdx)$/.test(name)) return 0;
  if (name === 'claude.md' || name === 'agents.md') return 1;
  if (!rel.includes('/')) return 2;
  if (name.startsWith('docs/') || name.startsWith('doc/')) return 3;
  return 4;
}

export async function listWorktreeMarkdown(root: string): Promise<MarkdownEntry[]> {
  const base = normalizePath(root).replace(/\/+$/, '');
  const paths = await fsListMarkdown(root);
  return paths
    .map((path) => {
      const rel = path.startsWith(`${base}/`) ? path.slice(base.length + 1) : path;
      return { path, rel, name: fileNameOf(path) };
    })
    .sort((a, b) => rank(a.rel) - rank(b.rel) || a.rel.localeCompare(b.rel));
}
