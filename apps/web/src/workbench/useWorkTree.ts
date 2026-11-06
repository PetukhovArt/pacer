import { computed, shallowRef, watch } from 'vue';
import { useExternalSessions } from '../agents/external';
import { useBerth } from '../agents/berth';
import { useMyIssues } from '../atlassian/myIssues';
import { useConnectors, type ChangeRequestRef, type InboxItem } from '../connectors/api';
import { itemId, useForgeInbox } from '../forge/inbox';
import { useSettings } from '../settings/store';
import { lookupRemotes, remotes } from './remotes';
import { fetchMissing, fetchedRows, parseMrUrl, refreshReviews, reviewEntries, syncReviewRepos } from './reviewQueue';
import { t } from '../i18n';
import { buildTree, tabCounts, type ReviewMeta, type ReviewNoteView, type ReviewSource, type ReviewState, type Sources, type TreeTab } from './tree';

const TAB_KEY = 'veyra.tree.tab';
const OLD_KEYS = ['veyra.tree.mode', 'veyra.tree.expanded.project', 'veyra.tree.expanded.type'];
const TABS: readonly TreeTab[] = ['sessions', 'mrs', 'reviews', 'tasks'];

function migrate() {
  const old = localStorage.getItem('veyra.tree.expanded.type');
  if (old)
    for (const tab of ['sessions', 'mrs', 'tasks'])
      if (localStorage.getItem(`veyra.tree.expanded.${tab}`) === null) localStorage.setItem(`veyra.tree.expanded.${tab}`, old);
  for (const key of OLD_KEYS) localStorage.removeItem(key);
}

migrate();

const HOSTS_KEY = 'veyra.review.hosts';

function storedHosts(): string[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(HOSTS_KEY) ?? '[]');
    return Array.isArray(parsed) ? parsed.filter((host): host is string => typeof host === 'string') : [];
  } catch {
    return [];
  }
}

function hostOf(url: string): string | null {
  try {
    return new URL(url).hostname.toLowerCase();
  } catch {
    return null;
  }
}

export const treeQuery = shallowRef('');
export const treeTab = shallowRef<TreeTab>(TABS.find((tab) => tab === localStorage.getItem(TAB_KEY)) ?? 'sessions');

watch(treeTab, (tab) => localStorage.setItem(TAB_KEY, tab));

export type TabInfo = { id: TreeTab; label: string; count: number | undefined; tone: 'alert' | 'accent' | 'muted'; title: string };

export function useWorkTree() {
  const berth = useBerth();
  const external = useExternalSessions();
  const { connectors } = useConnectors();
  const inbox = useForgeInbox();
  const jira = useMyIssues();
  const { values } = useSettings();

  watch(
    () => [...(berth?.entities.value.projects.values() ?? [])].map((project) => project.repo_path),
    (repos) => lookupRemotes(repos),
    { immediate: true },
  );
  watch(jira.jiraConnectors, () => jira.refresh(false), { immediate: true });

  const projectRepos = computed(() => [...(berth?.entities.value.projects.values() ?? [])].map((project) => ({ id: project.id, repo: project.repo_path })));
  watch(
    () => projectRepos.value.map((project) => project.repo),
    (repos) => syncReviewRepos(repos),
    { immediate: true },
  );

  const reviewRows = computed(() => {
    const rows = new Map<string, { ref: ChangeRequestRef; meta: ReviewMeta; url: string; projectId: string }>();
    const unresolved = new Map<string, ReviewNoteView[]>();
    for (const { id, repo } of projectRepos.value) {
      const entry = reviewEntries.value.get(repo);
      if (!entry?.adapter) continue;
      for (const row of entry.rows) {
        const ref = parseMrUrl(row.url, connectors.value);
        if (!ref) {
          unresolved.set(id, [...(unresolved.get(id) ?? []), { refs: row.refs, text: t('workbench.review.noConnector', { url: row.url }) }]);
          continue;
        }
        const key = itemId(ref);
        const have = rows.get(key);
        if (have) have.meta.refs = [...new Set([...have.meta.refs, ...row.refs])];
        else rows.set(key, { ref, meta: { refs: [...row.refs], approved: row.approved }, url: row.url, projectId: id });
      }
    }
    return { rows, unresolved };
  });

  watch(
    [reviewRows, () => inbox.items.value, () => inbox.loadedOnce.value],
    () => {
      if (!inbox.loadedOnce.value) return;
      const known = new Set(inbox.items.value.map(itemId));
      const since = Math.max(0, ...[...reviewEntries.value.values()].map((entry) => entry.loadedAt));
      fetchMissing(new Map([...reviewRows.value.rows].map(([key, row]) => [key, row.ref])), known, since);
    },
    { immediate: true },
  );

  const reviewMrs = computed(() => {
    const known = new Set(inbox.items.value.map(itemId));
    const extra: InboxItem[] = [];
    for (const key of reviewRows.value.rows.keys()) {
      const item = fetchedRows.value.get(key)?.item;
      if (item && !known.has(key)) extra.push(item);
    }
    return extra.length ? [...inbox.items.value, ...extra] : inbox.items.value;
  });

  const reviewSettled = computed(() => projectRepos.value.length > 0 && projectRepos.value.every(({ repo }) => (reviewEntries.value.get(repo)?.loadedAt ?? 0) > 0));

  const review = computed<ReviewSource>(() => {
    const hosts = new Set<string>(berth !== null && !reviewSettled.value ? storedHosts() : []);
    const byProject = new Map<string, { notes: ReviewNoteView[]; state: ReviewState }>();
    const seenNotes = new Set<string>();
    for (const { id, repo } of projectRepos.value) {
      const entry = reviewEntries.value.get(repo);
      if (!entry || (!entry.adapter && !entry.error)) continue;
      const host = remotes.value.get(repo)?.host;
      if (host) hosts.add(host);
      const notes: ReviewNoteView[] = [];
      for (const note of [...entry.notes, ...(reviewRows.value.unresolved.get(id) ?? [])]) {
        const key = `${note.refs.join(',')}|${note.text}`;
        if (seenNotes.has(key)) continue;
        seenNotes.add(key);
        notes.push(note);
      }
      byProject.set(id, { notes, state: { loading: entry.loading, error: entry.error } });
    }
    const inInbox = new Set(inbox.items.value.map(itemId));
    const rows = new Map<string, ReviewMeta>();
    for (const [key, row] of reviewRows.value.rows) {
      rows.set(key, row.meta);
      const rowHost = hostOf(row.url);
      if (rowHost) hosts.add(rowHost);
      const fetched = fetchedRows.value.get(key);
      if (!inInbox.has(key) && !fetched?.item && fetched?.error) byProject.get(row.projectId)?.notes.push({ refs: row.meta.refs, text: `${row.url}: ${fetched.error}` });
    }
    return { rows, hosts, byProject };
  });

  watch(
    () => (reviewSettled.value ? [...review.value.hosts].sort().join(',') : null),
    (hosts) => {
      if (hosts !== null) localStorage.setItem(HOSTS_KEY, JSON.stringify(hosts ? hosts.split(',') : []));
    },
  );

  const sources = computed<Sources>(() => ({
    berth: berth !== null,
    workspaces: berth?.tree.value ?? [],
    entities: berth?.entities.value,
    external: external?.sessions.value ?? [],
    linked: external?.linked.value ?? new Map(),
    remotes: remotes.value,
    connectors: connectors.value,
    mrs: reviewMrs.value,
    issues: jira.issues.value,
    jiraProjects: values.value['tree.jiraProjects'],
    review: review.value,
  }));

  const counts = computed(() => tabCounts(sources.value));
  const tabs = computed<TabInfo[]>(() => {
    const c = counts.value;
    const list: TabInfo[] = [
      c.sessionsWaiting
        ? { id: 'sessions', label: t('workbench.tabSessions'), count: c.sessionsWaiting, tone: 'alert', title: t('workbench.tabWaiting', { count: c.sessionsWaiting }) }
        : { id: 'sessions', label: t('workbench.tabSessions'), count: c.sessionsRunning || undefined, tone: 'accent', title: t('workbench.tabRunning', { count: c.sessionsRunning }) },
      c.mrsAttention
        ? { id: 'mrs', label: t('workbench.tabMrs'), count: c.mrsAttention, tone: 'alert', title: t('workbench.tabAttention', { count: c.mrsAttention }) }
        : { id: 'mrs', label: t('workbench.tabMrs'), count: c.mrs || undefined, tone: 'muted', title: t('workbench.tabOpen', { count: c.mrs }) },
      { id: 'reviews', label: t('workbench.tabReviews'), count: c.reviews || undefined, tone: 'accent', title: t('workbench.tabReviewsPending', { count: c.reviews }) },
    ];
    if (c.tasks || jira.jiraConnectors.value.length) list.push({ id: 'tasks', label: t('workbench.tabTasks'), count: c.tasks || undefined, tone: 'muted', title: t('workbench.tabOpen', { count: c.tasks }) });
    return list;
  });
  const activeTab = computed<TreeTab>(() => (tabs.value.some((tab) => tab.id === treeTab.value) ? treeTab.value : 'sessions'));

  const nodes = computed(() => buildTree(sources.value, activeTab.value, treeQuery.value));
  const all = computed(() => buildTree(sources.value, 'all', ''));

  function refresh() {
    void inbox.load();
    refreshReviews();
    jira.refresh(true);
  }

  return { berth, nodes, all, tabs, activeTab, refresh, loading: computed(() => inbox.loading.value || jira.loading.value || [...reviewEntries.value.values()].some((entry) => entry.loading)), errors: computed(() => [...inbox.errors.value.map((e) => ({ connector: e.connector, message: e.message })), ...jira.errors.value]) };
}
