import { computed, shallowRef, watch, type ComputedRef } from 'vue';
import { externalKey, externalTarget, readTranscript, useExternalSessions, type ExternalSession } from '../agents/external';
import { useBerth, type SessionNode, type SessionStatus } from '../agents/berth';
import { t } from '../i18n';
import { useDock } from '../workbench/dock';

export type HotStatus = 'waiting' | 'working' | 'idle' | 'error';
type HotHarness = 'claude' | 'codex' | 'shell';
export type HotItem = {
  key: string;
  title: string;
  project: string | null;
  branch: string | null;
  harness: HotHarness;
  status: HotStatus;
  unseen: boolean;
  pinned: boolean;
  lastActivity: number | null;
  preview: string | null;
};

type Source = { harness: 'claude' | 'codex'; id: string };
type Raw = Omit<HotItem, 'pinned' | 'preview' | 'lastActivity'> & { live: boolean; seed: number | null; stamp: number; source: Source | null };
type Tracked = { seq: number; since: number; sig: string };

const PINS_KEY = 'berth.hot.pins';
const MRU_LIMIT = 50;
const PREVIEW_LIMIT = 12;
const PREVIEW_CHARS = 140;
const PREVIEW_DEBOUNCE_MS = 800;
const TOAST_MS = 1800;
const SECONDS_LIMIT = 1e11;

export const hotToast = shallowRef<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | undefined;

function showToast(text: string): void {
  hotToast.value = text;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    hotToast.value = null;
  }, TOAST_MS);
}

const needsYou = (item: Pick<HotItem, 'status' | 'unseen'>) => item.status === 'waiting' || item.unseen;

export function hotDotOf(item: Pick<HotItem, 'status' | 'harness'>): SessionStatus {
  if (item.status === 'waiting') return 'needs_feedback';
  if (item.status === 'working') return 'running';
  if (item.status === 'error') return 'terminated';
  return item.harness === 'shell' ? 'shell' : 'finished';
}

function readPins(): string[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(PINS_KEY) ?? '[]');
    return Array.isArray(parsed) ? parsed.filter((entry): entry is string => typeof entry === 'string') : [];
  } catch {
    return [];
  }
}

const toMillis = (value: number) => (value > 0 && value < SECONDS_LIMIT ? value * 1000 : value);

function fromExternal(status: ExternalSession['status']): HotStatus {
  return status === 'waiting' ? 'waiting' : status === 'working' ? 'working' : 'idle';
}

function previewOf(messages: readonly { role: string; text: string; tool: string | null }[]): string | null {
  for (let index = messages.length - 1; index >= 0; index--) {
    const message = messages[index];
    if (!message) continue;
    const text = message.text.replace(/\s+/g, ' ').trim();
    if (message.role === 'assistant' && text) return text.slice(0, PREVIEW_CHARS);
    if (message.tool) return message.tool;
  }
  return null;
}

function create() {
  const berth = useBerth();
  const external = useExternalSessions();
  const dock = useDock();
  const pins = shallowRef<string[]>(readPins());
  const tracked = shallowRef<ReadonlyMap<string, Tracked>>(new Map());
  const previews = shallowRef<ReadonlyMap<string, string>>(new Map());
  const focused = shallowRef<string[]>([]);
  let counter = 0;

  const raw = computed<Raw[]>(() => {
    const out: Raw[] = [];
    const entities = berth?.entities.value;
    const linked = external?.linked.value;
    for (const node of berth?.sessions.value.values() ?? []) {
      const path = dock.paths.value.get(node.key);
      const agent = 'Agent' in node.ref ? entities?.agents.get(node.id) : undefined;
      const link = agent ? undefined : linked?.get(node.key);
      let status: HotStatus = 'idle';
      let harness: HotHarness = 'shell';
      let source: Source | null = null;
      let seed: number | null = null;
      let stamp = 0;
      if (agent) {
        status = agent.status === 'needs_feedback' ? 'waiting' : agent.status === 'running' ? 'working' : agent.status === 'terminated' ? 'error' : 'idle';
        harness = agent.kind === 'claude' ? 'claude' : agent.kind === 'codex' ? 'codex' : 'shell';
        if (agent.session_id && (agent.kind === 'claude' || agent.kind === 'codex')) source = { harness: agent.kind, id: agent.session_id };
        seed = agent.status_changed_at ? toMillis(agent.status_changed_at) : null;
        stamp = agent.status_changed_at;
      } else if (link?.running) {
        status = fromExternal(link.status);
        harness = link.harness;
        source = { harness: link.harness, id: link.id };
        seed = link.updated_at;
        stamp = link.updated_at;
      }
      out.push({
        key: node.key,
        title: node.name,
        project: path?.project.name ?? null,
        branch: path?.worktree.branch ?? null,
        harness,
        status,
        unseen: node.unseen,
        live: node.status !== 'closed' && node.status !== 'disconnected',
        seed,
        stamp,
        source,
      });
    }
    for (const session of external?.anyByKey.value.values() ?? []) {
      const key = externalKey(session);
      if (!external?.byKey.value.has(key)) continue;
      const target = externalTarget(session, entities);
      out.push({
        key,
        title: session.title,
        project: target?.projectName ?? null,
        branch: target?.branch ?? session.branch,
        harness: session.harness,
        status: session.running ? fromExternal(session.status) : 'idle',
        unseen: false,
        live: session.running,
        seed: session.updated_at,
        stamp: session.updated_at,
        source: { harness: session.harness, id: session.id },
      });
    }
    return out;
  });

  watch(
    raw,
    (entries) => {
      const previous = tracked.value;
      const next = new Map<string, Tracked>();
      let changed = previous.size !== entries.length;
      const now = Date.now();
      for (const entry of entries) {
        const sig = `${entry.status}|${entry.unseen}`;
        const known = previous.get(entry.key);
        if (!known) {
          next.set(entry.key, { seq: ++counter, since: entry.seed ?? now, sig });
          changed = true;
        } else if (known.sig !== sig) {
          next.set(entry.key, { seq: known.seq, since: now, sig });
          changed = true;
        } else next.set(entry.key, known);
      }
      if (changed) tracked.value = next;
    },
    { immediate: true, flush: 'sync' },
  );

  const items = computed<HotItem[]>(() => {
    const pinned = pins.value;
    const meta = tracked.value;
    const rank = (item: Raw) => (needsYou(item) ? 0 : item.status === 'working' ? 1 : item.status === 'error' ? 2 : 3);
    const entries = raw.value
      .filter((entry) => entry.live || pinned.includes(entry.key))
      .map((entry) => {
        const info = meta.get(entry.key);
        const lastActivity = Math.max(entry.seed ?? 0, info?.since ?? 0) || null;
        const item: HotItem = {
          key: entry.key,
          title: entry.title,
          project: entry.project,
          branch: entry.branch,
          harness: entry.harness,
          status: entry.status,
          unseen: entry.unseen,
          pinned: pinned.includes(entry.key),
          lastActivity,
          preview: previews.value.get(entry.key) ?? null,
        };
        return { item, rank: rank(entry), since: info?.since ?? 0, seq: info?.seq ?? 0, pin: pinned.indexOf(entry.key) };
      });
    entries.sort((a, b) => {
      if (a.item.pinned !== b.item.pinned) return a.item.pinned ? -1 : 1;
      if (a.item.pinned) return a.pin - b.pin;
      if (a.rank !== b.rank) return a.rank - b.rank;
      if (a.rank === 0) return b.since - a.since;
      if (a.rank === 3) return (b.item.lastActivity ?? 0) - (a.item.lastActivity ?? 0) || a.seq - b.seq;
      return a.seq - b.seq;
    });
    return entries.map((entry) => entry.item);
  });

  const waiting = computed(() => items.value.filter(needsYou));
  const keys = computed(() => new Set(items.value.map((item) => item.key)));

  const canonical = (key: string | null) => (key ? (external?.terminalOf.value.get(key) ?? key) : null);

  watch(
    () => canonical(dock.activeKey.value),
    (key) => {
      if (!key) return;
      focused.value = [key, ...focused.value.filter((entry) => entry !== key)].slice(0, MRU_LIMIT);
    },
    { immediate: true },
  );

  const mru = computed(() => {
    const live = keys.value;
    const recent = focused.value.filter((key) => live.has(key));
    const known = new Set(recent);
    return [...recent, ...items.value.map((item) => item.key).filter((key) => !known.has(key))];
  });

  const fetched = new Map<string, string>();
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  let previewRun = 0;

  async function refreshPreviews() {
    const run = ++previewRun;
    const wanted = raw.value.filter((entry) => entry.live && entry.source).slice(0, PREVIEW_LIMIT);
    for (const entry of wanted) {
      if (run !== previewRun) return;
      const source = entry.source;
      if (!source) continue;
      const stamp = `${entry.stamp}|${entry.status}`;
      if (fetched.get(entry.key) === stamp) continue;
      fetched.set(entry.key, stamp);
      const text = await readTranscript(source, 6).then(previewOf, () => null);
      if (run !== previewRun || !text || previews.value.get(entry.key) === text) continue;
      previews.value = new Map(previews.value).set(entry.key, text);
    }
  }

  watch(
    () => raw.value.map((entry) => `${entry.key}@${entry.stamp}|${entry.status}|${entry.live}`).join(','),
    () => {
      clearTimeout(previewTimer);
      previewTimer = setTimeout(() => void refreshPreviews(), PREVIEW_DEBOUNCE_MS);
    },
    { immediate: true },
  );

  function open(key: string) {
    const node: SessionNode | undefined = berth?.sessions.value.get(key);
    if (node) {
      dock.openSession(node);
      return;
    }
    const session = external?.anyByKey.value.get(key);
    if (session) dock.openExternal(session);
  }

  function next() {
    const list = waiting.value;
    if (!list.length) {
      showToast(t('hot.toast.nobody'));
      return;
    }
    const current = canonical(dock.activeKey.value);
    const index = list.findIndex((item) => item.key === current);
    const target = list[(index + 1) % list.length];
    if (!target) return;
    open(target.key);
    if (list.length > 1) showToast(t('hot.toast.waiting', { count: list.length }));
  }

  function savePins(next: string[]) {
    pins.value = next;
    localStorage.setItem(PINS_KEY, JSON.stringify(next));
  }

  function pin(key: string, on: boolean) {
    const has = pins.value.includes(key);
    if (on && !has) savePins([...pins.value, key]);
    else if (!on && has) savePins(pins.value.filter((entry) => entry !== key));
  }

  function move(key: string, toIndex: number) {
    const visible = items.value.filter((item) => item.pinned).map((item) => item.key);
    const hidden = pins.value.filter((entry) => !visible.includes(entry));
    const order = visible.filter((entry) => entry !== key);
    order.splice(Math.max(0, Math.min(toIndex, order.length)), 0, key);
    savePins([...order, ...hidden]);
  }

  return {
    items: items as ComputedRef<HotItem[]>,
    waiting: waiting as ComputedRef<HotItem[]>,
    mru: mru as ComputedRef<string[]>,
    open,
    next,
    pin,
    move,
  };
}

let store: ReturnType<typeof create> | null = null;

export function useHot() {
  store ??= create();
  return store;
}
