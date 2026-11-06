import { shallowRef } from 'vue';
import {
  describeError,
  forgeChangeRequest,
  reviewAdapter,
  reviewQueue,
  type ChangeRequestRef,
  type ConnectorView,
  type InboxItem,
  type ReviewQueueNote,
  type ReviewQueueRow,
} from '../connectors/api';
import { connectorBase } from './remotes';

const REFRESH_MS = 5 * 60_000;

export type ReviewEntry = {
  adapter: string | null;
  rows: ReviewQueueRow[];
  notes: ReviewQueueNote[];
  error: string | null;
  loading: boolean;
  loadedAt: number;
};

export type Fetched = { item: InboxItem | null; error: string | null; at: number };

export const reviewEntries = shallowRef<ReadonlyMap<string, ReviewEntry>>(new Map());
export const fetchedRows = shallowRef<ReadonlyMap<string, Fetched>>(new Map());

let repos: readonly string[] = [];
let timer: ReturnType<typeof setInterval> | undefined;
const inflight = new Set<string>();

function patchEntry(repo: string, change: Partial<ReviewEntry>) {
  const current = reviewEntries.value.get(repo) ?? { adapter: null, rows: [], notes: [], error: null, loading: false, loadedAt: 0 };
  reviewEntries.value = new Map(reviewEntries.value).set(repo, { ...current, ...change });
}

const patchAll = (members: readonly string[], change: Partial<ReviewEntry>) => {
  for (const repo of members) patchEntry(repo, change);
};

async function runGroup(members: readonly string[]) {
  patchAll(members, { loading: true });
  try {
    const result = await reviewQueue(members[0] ?? '');
    patchAll(members, { adapter: result.adapter, rows: result.rows, notes: result.notes, error: null, loading: false, loadedAt: Date.now() });
  } catch (e) {
    patchAll(members, { error: describeError(e), loading: false, loadedAt: Date.now() });
  }
}

async function load(list: readonly string[]) {
  const busy = list.filter((repo) => !reviewEntries.value.get(repo)?.loading);
  patchAll(busy, { loading: true });
  const groups = new Map<string, string[]>();
  await Promise.all(
    busy.map(async (repo) => {
      try {
        const key = await reviewAdapter(repo);
        if (key === null) patchEntry(repo, { adapter: null, rows: [], notes: [], error: null, loading: false, loadedAt: Date.now() });
        else groups.set(key, [...(groups.get(key) ?? []), repo]);
      } catch (e) {
        patchEntry(repo, { error: describeError(e), loading: false, loadedAt: Date.now() });
      }
    }),
  );
  await Promise.all([...groups.values()].map((members) => runGroup(list.filter((repo) => members.includes(repo)))));
}

export function refreshReviews() {
  void load(repos);
}

export function syncReviewRepos(list: readonly string[]) {
  const added = list.filter((repo) => !repos.includes(repo));
  repos = list;
  if (added.length) void load(added);
  if (timer === undefined && list.length)
    timer = setInterval(() => {
      if (document.visibilityState === 'visible') refreshReviews();
    }, REFRESH_MS);
}

export function parseMrUrl(url: string, connectors: readonly ConnectorView[]): ChangeRequestRef | null {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  const host = parsed.hostname.toLowerCase();
  const path = decodeURIComponent(parsed.pathname).replace(/^\/+/, '');
  const cut = path.search(/\/(-\/)?merge_requests\/\d+/);
  if (cut < 0) return null;
  const number = Number(/merge_requests\/(\d+)/.exec(path)?.[1]);
  const full = path.slice(0, cut);
  for (const connector of connectors) {
    if (connector.kind !== 'gitlab') continue;
    const base = connectorBase(connector.base_url);
    if (!base || base.host !== host) continue;
    const prefix = base.path ? `${base.path}/` : '';
    if (prefix && !full.startsWith(prefix)) continue;
    return { connector: connector.id, project: full.slice(prefix.length), number };
  }
  return null;
}

export function fetchMissing(refs: ReadonlyMap<string, ChangeRequestRef>, known: ReadonlySet<string>, since: number) {
  for (const [id, ref] of refs) {
    if (known.has(id) || inflight.has(id)) continue;
    const have = fetchedRows.value.get(id);
    if (have && have.at >= since) continue;
    inflight.add(id);
    void forgeChangeRequest(ref)
      .then((found): Fetched => ({ item: { ...found, reasons: ['reviewer'] }, error: null, at: Date.now() }))
      .catch((e): Fetched => ({ item: have?.item ?? null, error: describeError(e), at: Date.now() }))
      .then((result) => {
        inflight.delete(id);
        fetchedRows.value = new Map(fetchedRows.value).set(id, result);
      });
  }
}
