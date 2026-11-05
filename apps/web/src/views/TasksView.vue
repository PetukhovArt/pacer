<script setup lang="ts">
import { computed, onMounted, reactive, shallowRef, watch } from 'vue';
import type { StatusCategory, Task } from '@veyra/protocol';
import { CircleCheck, CircleDashed, CircleDot, Folder, FolderOpen, ListTodo, RefreshCw, Search, SlidersHorizontal, Tag, Ticket } from 'lucide-vue-next';
import { VButton, VInputText, VMultiSelect, VSelect, VToggleSwitch } from '@ui/kit';
import { useClient, useClientState } from '@core/vue-bindings';
import JiraIssueView from '../atlassian/JiraIssueView.vue';
import JiraPersonAvatar from '../atlassian/PersonAvatar.vue';
import TypeIcon from '../atlassian/TypeIcon.vue';
import Avatar from '../Avatar.vue';
import { useMyIssues } from '../atlassian/myIssues';
import { useConnectors, type JiraIssueSummary } from '../connectors/api';
import { t } from '../i18n';
import { withViewTransition } from '../motion';
import { openSettings } from '../nav';
import { priorityLevel, priorityRank } from '../priority';
import SidePane from '../SidePane.vue';
import ExplorerTree from '../tree/ExplorerTree.vue';
import type { TreeNode } from '../tree/types';
import TaskView from './TaskView.vue';

type Filters = { releases: string[]; statuses: string[]; priorities: string[]; labels: string[]; assignee: string; text: string; hideClosed: boolean };
type Selection = { source: 'relay'; id: string } | { source: 'jira'; connector: string; key: string };
type NodeData = { task?: Task; issue?: JiraIssueSummary };
type Node = TreeNode<NodeData>;

const NO_RELEASE = 'none';
const NO_PRIORITY = 'none';
const CATEGORIES: readonly StatusCategory[] = ['todo', 'in_progress', 'done'];
const CATEGORY_ICON = { todo: CircleDashed, in_progress: CircleDot, done: CircleCheck } as const;
const SELECTION_KEY = 'veyra.tasks.open';
const PANEL_KEY = 'veyra.tasks.filtersOpen';
const defaults = (): Filters => ({ releases: [], statuses: [], priorities: [], labels: [], assignee: 'all', text: '', hideClosed: true });

const client = useClient();
const state = useClientState();
const { connectors, loading: connectorsLoading, list: listConnectors } = useConnectors();
const storageKey = () => `veyra.taskFilters.${state.value.me.id}.${state.value.workspaceId ?? ''}`;

function load(): Filters {
  try {
    return { ...defaults(), ...(JSON.parse(localStorage.getItem(storageKey()) ?? '{}') as Partial<Filters>) };
  } catch {
    return defaults();
  }
}

const filters = reactive<Filters>(load());
const panelOpen = shallowRef(localStorage.getItem(PANEL_KEY) === 'true');

watch(panelOpen, (open) => localStorage.setItem(PANEL_KEY, String(open)));

watch(
  () => state.value.workspaceId,
  () => Object.assign(filters, load()),
);

watch(
  () => state.value.releases,
  (releases) => {
    if (!releases.length) return;
    const known = new Set([...releases.map((r) => r.id), NO_RELEASE]);
    const kept = filters.releases.filter((id) => known.has(id));
    if (kept.length !== filters.releases.length) filters.releases = kept;
  },
);

watch(
  filters,
  (value) => {
    const snapshot = { ...value, releases: [...value.releases], statuses: [...value.statuses], priorities: [...value.priorities], labels: [...value.labels] };
    localStorage.setItem(storageKey(), JSON.stringify(snapshot));
  },
  { deep: true },
);

function reset() {
  Object.assign(filters, defaults());
}

const activeFilters = computed(
  () => filters.releases.length + filters.statuses.length + filters.priorities.length + filters.labels.length + (filters.assignee === 'all' ? 0 : 1) + (filters.hideClosed ? 0 : 1),
);

const sortedReleases = computed(() => [...state.value.releases].sort((a, b) => b.name.localeCompare(a.name, undefined, { numeric: true })));

const releaseOptions = computed(() => [...sortedReleases.value.map((r) => ({ label: r.name, value: r.id })), { label: t('tasks.noRelease'), value: NO_RELEASE }]);

const statusOptions = computed(() => [...new Set(state.value.tasks.map((task) => task.status))].sort((a, b) => a.localeCompare(b)));

const priorityOptions = computed(() => {
  const names = [...new Set([...state.value.tasks.flatMap((task) => (task.priority ? [task.priority] : [])), ...jiraIssues.value.flatMap((issue) => (issue.priority ? [issue.priority] : []))])].sort(
    (a, b) => priorityRank(a) - priorityRank(b) || a.localeCompare(b),
  );
  return [...names.map((n) => ({ label: n, value: n })), { label: t('tasks.filters.noPriority'), value: NO_PRIORITY }];
});

const labelOptions = computed(() => [...new Set(state.value.tasks.flatMap((task) => task.labels ?? []))].sort((a, b) => a.localeCompare(b)));

function byPriority(a: Task, b: Task) {
  return priorityRank(a.priority) - priorityRank(b.priority) || a.externalRef.localeCompare(b.externalRef, undefined, { numeric: true });
}

const assigneeOptions = computed(() => {
  const ids = new Set(state.value.tasks.flatMap((task) => (task.assigneeId ? [task.assigneeId] : [])));
  const all = [...ids].map((id) => ({ label: client.actorName(id), value: `actor:${id}` })).sort((a, b) => a.label.localeCompare(b.label));
  return [
    { label: t('tasks.filters.assigneeAll'), value: 'all' },
    { label: t('tasks.filters.assigneeMine'), value: 'mine' },
    { label: t('tasks.filters.assigneeUnassigned'), value: 'unassigned' },
    ...all,
  ];
});

const textQuery = computed(() => filters.text.trim().toLowerCase());

function textMatches(key: string, title: string) {
  const text = textQuery.value;
  return !text || key.toLowerCase().includes(text) || title.toLowerCase().includes(text);
}

function matches(task: Task) {
  if (filters.hideClosed && task.statusCategory === 'done') return false;
  if (filters.releases.length && !filters.releases.includes(task.releaseId ?? NO_RELEASE)) return false;
  if (filters.statuses.length && !filters.statuses.includes(task.status)) return false;
  if (filters.priorities.length && !filters.priorities.includes(task.priority ?? NO_PRIORITY)) return false;
  if (filters.labels.length && !filters.labels.some((l) => task.labels?.includes(l))) return false;
  if (filters.assignee === 'mine' && task.assigneeId !== state.value.me.id) return false;
  if (filters.assignee === 'unassigned' && task.assigneeId) return false;
  if (filters.assignee.startsWith('actor:') && task.assigneeId !== filters.assignee.slice(6)) return false;
  return textMatches(task.externalRef, task.title);
}

function issueMatches(issue: JiraIssueSummary) {
  if (filters.hideClosed && issue.status_category === 'done') return false;
  if (filters.priorities.length && !filters.priorities.includes(issue.priority ?? NO_PRIORITY)) return false;
  return textMatches(issue.key, issue.summary);
}

const { jiraConnectors, issues: jiraIssues, errors: jiraErrors, loading: jiraLoading, refresh: refreshJira } = useMyIssues();

const visible = computed(() => state.value.tasks.filter(matches));
const visibleIssues = computed(() => jiraIssues.value.filter(issueMatches));
const total = computed(() => state.value.tasks.length + jiraIssues.value.length);

const settled = computed(() => connectors.value.length > 0 || !connectorsLoading.value);
const noSource = computed(() => settled.value && !state.value.tasks.length && !jiraConnectors.value.length);

watch(jiraConnectors, () => refreshJira(false));

onMounted(() => {
  if (!connectors.value.length) void listConnectors();
  refreshJira(false);
});

const mrCounts = computed(() => {
  const counts = new Map<string, number>();
  for (const mr of state.value.mergeRequests) for (const ref of mr.taskRefs) counts.set(ref, (counts.get(ref) ?? 0) + 1);
  return counts;
});

function mrCount(task: Task) {
  return mrCounts.value.get(task.id) ?? 0;
}

function group(id: string, kind: string, label: string, children: Node[], extra: Partial<Node> = {}): Node {
  const count = countLeaves(children);
  return { id, kind, label, children, count, countTitle: t('tasks.tree.countTitle', { count }), ...extra };
}

function countLeaves(nodes: Node[]): number {
  return nodes.reduce((sum, node) => sum + (node.children ? countLeaves(node.children) : 1), 0);
}

function categoryNodes<I>(prefix: string, items: I[], categoryOf: (item: I) => StatusCategory, leaf: (item: I) => Node): Node[] {
  return CATEGORIES.flatMap((category) => {
    const list = items.filter((item) => categoryOf(item) === category);
    if (!list.length) return [];
    return [group(`${prefix}|cat:${category}`, 'category', t(`tasks.tree.category.${category}`), list.map(leaf), { icon: CATEGORY_ICON[category], defaultExpanded: category !== 'done' })];
  });
}

const taskLeaf = (task: Task): Node => ({ id: `task:${task.id}`, kind: 'task', label: `${task.externalRef} ${task.title}`, title: task.title, data: { task } });
const issueLeaf = (issue: JiraIssueSummary): Node => ({ id: `issue:${issue.connector}:${issue.key}`, kind: 'issue', label: `${issue.key} ${issue.summary}`, title: issue.summary, data: { issue } });

const relayNodes = computed<Node[]>(() => {
  const tasks = [...visible.value].sort(byPriority);
  const known = new Set(sortedReleases.value.map((r) => r.id));
  const releases = [...sortedReleases.value.map((r) => ({ id: r.id, name: r.name })), { id: NO_RELEASE, name: t('tasks.noRelease') }];
  return releases.flatMap((release) => {
    const list = tasks.filter((task) => (release.id === NO_RELEASE ? !task.releaseId || !known.has(task.releaseId) : task.releaseId === release.id));
    if (!list.length) return [];
    const id = `rel:${release.id}`;
    return [group(id, 'release', release.name, categoryNodes(id, list, (task) => task.statusCategory ?? 'todo', taskLeaf), { icon: Tag })];
  });
});

function projectOf(key: string) {
  const dash = key.lastIndexOf('-');
  return dash > 0 ? key.slice(0, dash) : key;
}

const jiraNodes = computed<Node[]>(() => {
  const perConnector = jiraConnectors.value.flatMap((connector) => {
    const issues = visibleIssues.value.filter((issue) => issue.connector === connector.id);
    if (!issues.length) return [];
    const prefix = `jira:${connector.id}`;
    const projects = [...new Set(issues.map((issue) => projectOf(issue.key)))].sort((a, b) => a.localeCompare(b));
    const children = projects.map((project) =>
      group(
        `${prefix}|p:${project}`,
        'project',
        project,
        categoryNodes(
          `${prefix}|p:${project}`,
          issues.filter((issue) => projectOf(issue.key) === project),
          (issue) => issue.status_category,
          issueLeaf,
        ),
        { icon: Folder, iconOpen: FolderOpen },
      ),
    );
    return [{ connector, children }];
  });
  if (perConnector.length === 1) return perConnector[0]!.children;
  return perConnector.map(({ connector, children }) => group(`jira:${connector.id}`, 'host', connector.label, children, { icon: Ticket }));
});

const nodes = computed<Node[]>(() => {
  const relay = relayNodes.value;
  const jira = jiraNodes.value;
  if (!relay.length || !jiraConnectors.value.length) return relay.length ? relay : jira;
  return [group('src:relay', 'source', t('tasks.tree.relay'), relay, { icon: ListTodo }), ...(jira.length ? [group('src:jira', 'source', t('tasks.tree.jira'), jira, { icon: Ticket })] : [])];
});

const shown = computed(() => visible.value.length + visibleIssues.value.length);

function readSelection(): Selection | null {
  try {
    const raw = JSON.parse(localStorage.getItem(SELECTION_KEY) ?? 'null') as Partial<{ source: string; id: string; connector: string; key: string }> | null;
    if (raw?.source === 'relay' && typeof raw.id === 'string') return { source: 'relay', id: raw.id };
    if (raw?.source === 'jira' && typeof raw.connector === 'string' && typeof raw.key === 'string') return { source: 'jira', connector: raw.connector, key: raw.key };
  } catch {
    return null;
  }
  return null;
}

const selection = shallowRef<Selection | null>(readSelection());

watch(selection, (value) => localStorage.setItem(SELECTION_KEY, JSON.stringify(value)));

const selectedId = computed(() => {
  const value = selection.value;
  if (!value) return null;
  return value.source === 'relay' ? `task:${value.id}` : `issue:${value.connector}:${value.key}`;
});

const relaySelected = computed(() => {
  const value = selection.value;
  return value?.source === 'relay' && state.value.tasks.some((task) => task.id === value.id) ? value : null;
});
const jiraSelected = computed(() => {
  const value = selection.value;
  return value?.source === 'jira' && jiraConnectors.value.some((c) => c.id === value.connector) ? value : null;
});

function select(next: Selection) {
  void withViewTransition(() => {
    selection.value = next;
  });
}

function onOpen(node: Node) {
  const { task, issue } = node.data ?? {};
  if (task) select({ source: 'relay', id: task.id });
  else if (issue) select({ source: 'jira', connector: issue.connector, key: issue.key });
}
</script>

<template>
  <section class="page" data-testid="tasks-view" :aria-label="t('tasks.title')">
    <header class="header">
      <h1><ListTodo :size="18" />{{ t('tasks.title') }}</h1>
      <span v-if="!noSource" class="vr-muted" data-testid="tasks-count">{{ t('tasks.count', { shown, total }) }}</span>
    </header>

    <div v-if="noSource" class="state empty" data-testid="tasks-connect">
      <ListTodo :size="32" />
      <strong>{{ t('tasks.tree.connect.title') }}</strong>
      <p>{{ t('tasks.tree.connect.text') }}</p>
      <VButton :label="t('tasks.tree.connect.action')" size="small" data-testid="tasks-connect-open" @click="openSettings('connectors')" />
    </div>

    <div v-else class="split">
      <SidePane storage-key="veyra.tasks.width" :default-width="380" :label="t('tasks.tree.listWidth')" handle-testid="tasks-resize-handle">
        <div class="list">
          <div class="bar">
            <div class="search">
              <Search :size="14" />
              <VInputText v-model="filters.text" class="search-input" :placeholder="t('common.search')" size="small" autocomplete="off" data-testid="filter-text" />
            </div>
            <button
              class="icon-button"
              :aria-pressed="panelOpen"
              :aria-label="t('tasks.tree.filters')"
              :title="activeFilters ? t('tasks.tree.activeFilters', { count: activeFilters }) : t('tasks.tree.filters')"
              data-testid="filter-toggle"
              @click="panelOpen = !panelOpen"
            >
              <SlidersHorizontal :size="14" />
              <span v-if="activeFilters" class="badge" data-testid="filter-active-count">{{ activeFilters }}</span>
            </button>
            <VButton
              v-if="jiraConnectors.length"
              size="small"
              severity="secondary"
              text
              :loading="jiraLoading"
              :aria-label="t('tasks.tree.refresh')"
              :title="t('tasks.tree.refresh')"
              data-testid="tasks-refresh"
              @click="refreshJira(true)"
            >
              <template #icon><RefreshCw :size="14" /></template>
            </VButton>
          </div>

          <div v-if="panelOpen" class="filters" data-testid="tasks-filters">
            <VMultiSelect
              v-model="filters.releases"
              :options="releaseOptions"
              option-label="label"
              option-value="value"
              :placeholder="t('tasks.filters.release')"
              filter
              :max-selected-labels="2"
              :selected-items-label="t('tasks.filters.selected', { value: '{0}' })"
              size="small"
              data-testid="filter-release"
            />
            <VMultiSelect
              v-model="filters.statuses"
              :options="statusOptions"
              :placeholder="t('tasks.filters.status')"
              filter
              :max-selected-labels="2"
              :selected-items-label="t('tasks.filters.selected', { value: '{0}' })"
              size="small"
              data-testid="filter-status"
            />
            <VMultiSelect
              v-model="filters.priorities"
              :options="priorityOptions"
              option-label="label"
              option-value="value"
              :placeholder="t('tasks.filters.priority')"
              :max-selected-labels="2"
              :selected-items-label="t('tasks.filters.selected', { value: '{0}' })"
              size="small"
              data-testid="filter-priority"
            />
            <VMultiSelect
              v-model="filters.labels"
              :options="labelOptions"
              :placeholder="t('tasks.filters.labels')"
              filter
              :max-selected-labels="2"
              :selected-items-label="t('tasks.filters.selected', { value: '{0}' })"
              size="small"
              data-testid="filter-labels"
            />
            <VSelect v-model="filters.assignee" :options="assigneeOptions" option-label="label" option-value="value" filter size="small" data-testid="filter-assignee" />
            <div class="filters-foot">
              <label class="toggle">
                <VToggleSwitch v-model="filters.hideClosed" data-testid="filter-hide-closed" />
                {{ t('tasks.filters.hideClosed') }}
              </label>
              <VButton :label="t('tasks.filters.reset')" size="small" text data-testid="filter-reset" @click="reset" />
            </div>
          </div>

          <div v-for="error in jiraErrors" :key="error.connector" class="strip" role="alert" data-testid="tasks-jira-error">{{ t('tasks.tree.jiraError', error) }}</div>

          <div class="scroll">
            <ExplorerTree
              v-if="nodes.length"
              :nodes="nodes"
              storage-key="veyra.tasks.tree.expanded"
              :label="t('tasks.tree.label')"
              :selected="selectedId"
              :reveal-all="textQuery.length > 0"
              test-id="tasks-tree"
              @open="onOpen"
            >
              <template #leaf="{ data }">
                <span v-if="data?.task" class="row" :data-testid="`task-${data.task.externalRef}`">
                  <i class="priority" :data-level="priorityLevel(data.task.priority)" :title="data.task.priority ?? t('tasks.filters.noPriority')" data-testid="task-priority" />
                  <strong class="ref">{{ data.task.externalRef }}</strong>
                  <span class="title">{{ data.task.title }}</span>
                  <span v-if="mrCount(data.task)" class="vr-chip mr" :title="t('tasks.filters.mergeRequests', { count: mrCount(data.task) })" data-testid="task-mr-count">MR {{ mrCount(data.task) }}</span>
                  <Avatar v-if="data.task.assigneeId" :id="data.task.assigneeId" :name="client.actorName(data.task.assigneeId)" small :title="client.actorName(data.task.assigneeId)" />
                </span>
                <span v-else-if="data?.issue" class="row" :data-testid="`issue-${data.issue.key}`">
                  <i class="priority" :data-level="priorityLevel(data.issue.priority)" :title="data.issue.priority ?? t('tasks.filters.noPriority')" />
                  <TypeIcon :type="data.issue.issue_type" />
                  <strong class="ref">{{ data.issue.key }}</strong>
                  <span class="title">{{ data.issue.summary }}</span>
                  <JiraPersonAvatar v-if="data.issue.assignee" :connector="data.issue.connector" :person="data.issue.assignee" small />
                </span>
              </template>
            </ExplorerTree>
            <p v-if="jiraLoading && !jiraIssues.length" class="note" data-testid="tasks-jira-loading">{{ t('tasks.tree.loadingJira') }}</p>
            <div v-if="!nodes.length && !jiraLoading" class="note" data-testid="tasks-empty">
              <template v-if="total">
                <span>{{ t('tasks.filters.emptyFiltered') }}</span>
                <VButton :label="t('tasks.filters.reset')" size="small" text data-testid="empty-reset" @click="reset" />
              </template>
              <span v-else>{{ t('tasks.filters.empty') }}</span>
            </div>
          </div>
        </div>
      </SidePane>
      <div class="detail vt-detail">
        <TaskView v-if="relaySelected" :key="relaySelected.id" :task-id="relaySelected.id" embedded />
        <JiraIssueView
          v-else-if="jiraSelected"
          :key="`${jiraSelected.connector}/${jiraSelected.key}`"
          :connector="jiraSelected.connector"
          :issue-key="jiraSelected.key"
          @open="(target) => select({ source: 'jira', connector: target.connector, key: target.key })"
        />
        <p v-else class="placeholder" data-testid="tasks-placeholder">{{ t('tasks.tree.placeholder') }}</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.page {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  color: var(--vr-color-text);
}

.header {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) var(--vr-space-6);
  border-bottom: 1px solid var(--vr-color-border);
}

h1 {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
}

.state.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--vr-space-3);
  max-width: 28rem;
  margin: auto;
  color: var(--vr-color-text-muted);
  text-align: center;
}

.state.empty strong {
  color: var(--vr-color-text);
}

.state.empty p {
  margin: 0;
}

.split {
  display: flex;
  flex: 1;
  min-height: 0;
}

.list {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  font-size: var(--vr-font-size-sm);
}

.bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.search {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-2);
  color: var(--vr-color-text-muted);
}

.search-input {
  flex: 1;
  min-width: 0;
}

.icon-button {
  position: relative;
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.icon-button:hover,
.icon-button[aria-pressed='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.badge {
  position: absolute;
  top: 0;
  right: 0;
  min-width: 14px;
  padding: 0 3px;
  font-size: var(--vr-font-size-2xs);
  line-height: 14px;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
  border-radius: var(--vr-radius-full);
}

.filters {
  display: grid;
  flex: none;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.filters > * {
  min-width: 0;
}

.filters-foot {
  display: flex;
  grid-column: 1 / -1;
  align-items: center;
  justify-content: space-between;
}

.toggle {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
}

.strip {
  flex: none;
  padding: var(--vr-space-1) var(--vr-space-3);
  color: var(--vr-color-danger);
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-chip-bg);
  border-bottom: 1px solid var(--vr-color-border);
}

.scroll {
  flex: 1;
  min-height: 0;
  padding: var(--vr-space-2);
  overflow: auto;
}

.note {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: var(--vr-space-4) var(--vr-space-2);
  color: var(--vr-color-text-muted);
}

.row {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.ref {
  flex: none;
  font-weight: 600;
  font-size: var(--vr-font-size-xs);
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.mr {
  flex: none;
}

.priority {
  flex: none;
  width: 8px;
  height: 8px;
  background: var(--vr-color-border);
  border-radius: var(--vr-radius-full);
}

.priority[data-level='blocker'],
.priority[data-level='high'] {
  background: var(--vr-color-danger);
}

.priority[data-level='medium'] {
  background: var(--vr-color-conflict);
}

.priority[data-level='low'] {
  background: var(--vr-color-agent-running);
}

.placeholder {
  margin: auto;
  color: var(--vr-color-text-muted);
}

.detail {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
</style>
