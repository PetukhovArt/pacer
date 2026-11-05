import { computed, shallowRef, type ComputedRef, type ShallowRef } from 'vue';
import { describeError, forgeInbox, type ChangeRequestRef, type InboxError, type InboxItem, type PipelineStatus } from '../connectors/api';
import { t } from '../i18n';
import type { TreeNode } from '../tree/types';
import { numberLabel } from './format';

type InboxReason = InboxItem['reasons'][number];

const REFRESH_MS = 2 * 60_000;
const TICK_MS = 30_000;

type InboxStore = {
  items: ShallowRef<InboxItem[]>;
  errors: ShallowRef<InboxError[]>;
  failure: ShallowRef<string | null>;
  loading: ShallowRef<boolean>;
  loadedOnce: ShallowRef<boolean>;
  loadedAt: ShallowRef<number>;
  reviewCount: ComputedRef<number>;
  load: () => Promise<void>;
  loadIfStale: (maxAgeMs: number) => void;
  startPolling: () => () => void;
};

let store: InboxStore | null = null;

function createStore(): InboxStore {
  const items = shallowRef<InboxItem[]>([]);
  const errors = shallowRef<InboxError[]>([]);
  const failure = shallowRef<string | null>(null);
  const loading = shallowRef(false);
  const loadedOnce = shallowRef(false);
  const loadedAt = shallowRef(0);
  const reviewCount = computed(() => items.value.filter((item) => item.reasons.includes('reviewer')).length);

  async function load() {
    if (loading.value) return;
    loading.value = true;
    try {
      const inbox = await forgeInbox();
      items.value = inbox.items;
      errors.value = inbox.errors;
      failure.value = null;
    } catch (e) {
      failure.value = describeError(e);
    } finally {
      loading.value = false;
      loadedOnce.value = true;
      loadedAt.value = Date.now();
    }
  }

  function loadIfStale(maxAgeMs: number) {
    if (Date.now() - loadedAt.value >= maxAgeMs) void load();
  }

  let pollers = 0;
  let timer: ReturnType<typeof setInterval> | undefined;

  function tick() {
    if (document.visibilityState === 'visible') loadIfStale(REFRESH_MS);
  }

  function startPolling() {
    if (pollers++ === 0) {
      tick();
      timer = setInterval(tick, TICK_MS);
      document.addEventListener('visibilitychange', tick);
    }
    let stopped = false;
    return () => {
      if (stopped) return;
      stopped = true;
      if (--pollers === 0) {
        clearInterval(timer);
        document.removeEventListener('visibilitychange', tick);
      }
    };
  }

  return { items, errors, failure, loading, loadedOnce, loadedAt, reviewCount, load, loadIfStale, startPolling };
}

export function useForgeInbox(): InboxStore {
  store ??= createStore();
  return store;
}

type InboxNodeData = { item?: InboxItem; pipeline?: PipelineStatus; group?: string; name?: string };
export type InboxNode = TreeNode<InboxNodeData>;

const ROLES = [
  { reason: 'reviewer', title: 'forge.inbox.groupReviewer' },
  { reason: 'author', title: 'forge.inbox.groupAuthor' },
  { reason: 'assignee', title: 'forge.inbox.groupAssignee' },
] as const;

const ROLLUP: readonly PipelineStatus[] = ['failed', 'running', 'pending', 'success'];

export const itemId = (ref: ChangeRequestRef) => `mr:${ref.connector}/${ref.project}/${ref.number}`;

function primaryRole(item: InboxItem): InboxReason {
  return ROLES.find((role) => item.reasons.includes(role.reason))?.reason ?? 'assignee';
}

function rollup(items: InboxItem[]): PipelineStatus | undefined {
  return ROLLUP.find((status) => items.some((item) => item.pipeline === status));
}

function groupBy<K>(items: InboxItem[], key: (item: InboxItem) => K): Map<K, InboxItem[]> {
  const map = new Map<K, InboxItem[]>();
  for (const item of items) map.set(key(item), [...(map.get(key(item)) ?? []), item]);
  return map;
}

function groupNode(id: string, kind: string, label: string, items: InboxItem[], children: InboxNode[], extra: InboxNodeData = {}): InboxNode {
  return { id, kind, label, children, count: items.length, countTitle: t('forge.inbox.countTitle', { count: items.length }), data: { pipeline: rollup(items), ...extra } };
}

function projectNodes(prefix: string, items: InboxItem[]): InboxNode[] {
  return [...groupBy(items, (item) => item.project)]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([project, list]) => {
      const cut = project.lastIndexOf('/');
      const leaves = list.map((item): InboxNode => ({ id: itemId(item), kind: 'mr', label: item.title, title: `${item.project} ${numberLabel(item)}`, data: { item } }));
      return groupNode(`${prefix}|p:${project}`, 'project', project, list, leaves, { group: cut > 0 ? project.slice(0, cut) : undefined, name: project.slice(cut + 1) });
    });
}

export function inboxTree(items: InboxItem[], connectorLabel: (id: string) => string): InboxNode[] {
  const sorted = [...items].sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at));
  const byRole = groupBy(sorted, primaryRole);
  return ROLES.flatMap((role) => {
    const list = byRole.get(role.reason);
    if (!list?.length) return [];
    const sectionId = `role:${role.reason}`;
    const byHost = groupBy(list, (item) => item.connector);
    const hosts = [...byHost].map(([connector, hostItems]) => {
      const hostId = `${sectionId}|h:${connector}`;
      return { hostId, connector, hostItems, projects: projectNodes(hostId, hostItems) };
    });
    const children =
      hosts.length > 1 ? hosts.map((host) => groupNode(host.hostId, 'host', connectorLabel(host.connector), host.hostItems, host.projects)) : hosts.flatMap((host) => host.projects);
    return [groupNode(sectionId, 'section', t(role.title), list, children)];
  });
}
