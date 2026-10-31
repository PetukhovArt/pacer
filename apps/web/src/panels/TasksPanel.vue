<script setup lang="ts">
import { computed, onUnmounted, reactive, ref, watch } from 'vue';
import type { IDockviewPanelProps } from 'dockview-core';
import type { Task } from '@veyra/protocol';
import { VButton, VInputText, VMultiSelect, VSelect, VToggleSwitch } from '@ui/kit';
import { useClient, useClientState } from '@core/vue-bindings';
import { openTask } from '../open-task';
import { priorityRank } from '../priority';
import TaskMeta from './TaskMeta.vue';

type Filters = { releases: string[]; statuses: string[]; priorities: string[]; labels: string[]; assignee: string; text: string; hideClosed: boolean };

const NO_RELEASE = 'none';
const NO_PRIORITY = 'none';
const defaults = (): Filters => ({ releases: [], statuses: [], priorities: [], labels: [], assignee: 'all', text: '', hideClosed: true });

const props = defineProps<{ params: IDockviewPanelProps }>();
const client = useClient();
const state = useClientState();
const storageKey = () => `veyra.taskFilters.${state.value.me?.id}.${state.value.workspaceId ?? ''}`;

function load(): Filters {
  const saved = props.params.params as { filters?: Partial<Filters>; workspaceId?: string | null } | undefined;
  if (saved?.filters && saved.workspaceId === state.value.workspaceId) return { ...defaults(), ...saved.filters };
  try {
    return { ...defaults(), ...(JSON.parse(localStorage.getItem(storageKey()) ?? '{}') as Partial<Filters>) };
  } catch {
    return defaults();
  }
}

const filters = reactive<Filters>(load());

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
    props.params.api.updateParameters({ filters: snapshot, workspaceId: state.value.workspaceId });
  },
  { deep: true },
);

function reset() {
  Object.assign(filters, defaults());
}

const sortedReleases = computed(() => [...state.value.releases].sort((a, b) => b.name.localeCompare(a.name, undefined, { numeric: true })));

const releaseOptions = computed(() => [...sortedReleases.value.map((r) => ({ label: r.name, value: r.id })), { label: 'Без релиза', value: NO_RELEASE }]);

const statusOptions = computed(() => [...new Set(state.value.tasks.map((t) => t.status))].sort((a, b) => a.localeCompare(b)));

const priorityOptions = computed(() => {
  const names = [...new Set(state.value.tasks.flatMap((t) => (t.priority ? [t.priority] : [])))].sort((a, b) => priorityRank(a) - priorityRank(b) || a.localeCompare(b));
  return [...names.map((n) => ({ label: n, value: n })), { label: 'Без приоритета', value: NO_PRIORITY }];
});

const labelOptions = computed(() => [...new Set(state.value.tasks.flatMap((t) => t.labels ?? []))].sort((a, b) => a.localeCompare(b)));

function byPriority(a: Task, b: Task) {
  return priorityRank(a.priority) - priorityRank(b.priority) || a.externalRef.localeCompare(b.externalRef, undefined, { numeric: true });
}

const trackerUser =computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId)?.trackerUser ?? null);

const assigneeOptions = computed(() => {
  const ids = new Set(state.value.tasks.flatMap((t) => (t.assigneeId ? [t.assigneeId] : [])));
  for (const actor of state.value.actors.values()) if (actor.kind === 'human') ids.add(actor.id);
  const people = [...ids].map((id) => ({ label: client.actorName(id), value: `actor:${id}` }));
  const refs = new Map(state.value.tasks.flatMap((t) => (t.assigneeRef ? [[t.assigneeRef, t.assigneeName ?? t.assigneeRef] as const] : [])));
  const trackerPeople = [...refs].map(([ref, name]) => ({ label: name, value: `ref:${ref}` }));
  const all = [...people, ...trackerPeople].sort((a, b) => a.label.localeCompare(b.label));
  return [{ label: 'Все', value: 'all' }, { label: 'Мои', value: 'mine' }, { label: 'Не назначены', value: 'unassigned' }, ...all];
});

function isMine(task: Task) {
  return task.assigneeId === state.value.me?.id || (!!trackerUser.value && task.assigneeRef === trackerUser.value);
}

function assigneeLabel(task: Task) {
  if (task.assigneeId) return client.actorName(task.assigneeId);
  return task.assigneeName ?? 'не назначена';
}

function isClosed(task: Task) {
  return task.statusCategory === 'done';
}

function matches(task: Task) {
  if (filters.hideClosed && isClosed(task)) return false;
  if (filters.releases.length && !filters.releases.includes(task.releaseId ?? NO_RELEASE)) return false;
  if (filters.statuses.length && !filters.statuses.includes(task.status)) return false;
  if (filters.priorities.length && !filters.priorities.includes(task.priority ?? NO_PRIORITY)) return false;
  if (filters.labels.length && !filters.labels.some((l) => task.labels?.includes(l))) return false;
  if (filters.assignee === 'mine'&& !isMine(task)) return false;
  if (filters.assignee === 'unassigned' && (task.assigneeId || task.assigneeRef)) return false;
  if (filters.assignee.startsWith('ref:') && task.assigneeRef !== filters.assignee.slice(4)) return false;
  if (filters.assignee.startsWith('actor:') && task.assigneeId !== filters.assignee.slice(6)) return false;
  const text = filters.text.trim().toLowerCase();
  return !text || task.externalRef.toLowerCase().includes(text) || task.title.toLowerCase().includes(text);
}

const visible = computed(() => state.value.tasks.filter(matches));

const groups = computed(() => {
  const tasks = [...visible.value].sort(byPriority);
  const byRelease = sortedReleases.value.map((r) => ({ id: r.id, name: r.name, tasks: tasks.filter((t) => t.releaseId === r.id) }));
  const known = new Set(sortedReleases.value.map((r) => r.id));
  const none = { id: NO_RELEASE, name: 'Без релиза', tasks: tasks.filter((t) => !t.releaseId || !known.has(t.releaseId)) };
  return [...byRelease, none].filter((g) => g.tasks.length);
});

const mrCounts = computed(() => {
  const counts = new Map<string, number>();
  for (const mr of state.value.mergeRequests) for (const ref of mr.taskRefs) counts.set(ref, (counts.get(ref) ?? 0) + 1);
  return counts;
});

function mrCount(task: Task) {
  return mrCounts.value.get(task.externalRef) ?? 0;
}

function agentStatus(id: string) {
  const actor = state.value.actors.get(id);
  return actor?.kind === 'agent' ? actor.status : null;
}

function open(task: Task) {
  openTask(props.params.containerApi, task);
}

const selectedId = ref<string | null>(null);

function trackActive(panelId: string | undefined) {
  if (panelId?.startsWith('task:')) selectedId.value = panelId.slice(5);
}

trackActive(props.params.containerApi.activePanel?.id);
const activeSub = props.params.containerApi.onDidActivePanelChange((panel) => trackActive(panel?.id));
const removeSub = props.params.containerApi.onDidRemovePanel((panel) => {
  if (panel.id === `task:${selectedId.value}`) selectedId.value = null;
});
onUnmounted(() => {
  activeSub.dispose();
  removeSub.dispose();
});

const workspace = computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId));
const now = ref(Date.now());
const ticker = setInterval(() => (now.value = Date.now()), 30_000);
onUnmounted(() => clearInterval(ticker));
const syncing = ref(false);

function ago(at: string) {
  const minutes = Math.floor((now.value - new Date(at).getTime()) / 60_000);
  if (minutes < 1) return 'только что';
  if (minutes < 60) return `${minutes} мин назад`;
  return `${Math.floor(minutes / 60)} ч назад`;
}

function failure(error: string | undefined) {
  return `${error ?? 'ошибка'}${error === 'timeout' ? ' — проверь VPN' : ''}`;
}

const syncLine = computed(() => {
  const w = workspace.value;
  if (!w?.trackerKind) return null;
  const kind = w.trackerKind === 'jira' ? 'Jira' : 'GitLab';
  const tracker = w.sync?.tracker;
  const mr = w.sync?.mergeRequests;
  const sameFailure = tracker && !tracker.ok && mr && !mr.ok && mr.error === tracker.error;
  const mrError = mr && !mr.ok && !sameFailure ? ` · MR: ${failure(mr.error)}` : '';
  if (!tracker) return { ok: !mrError, text: `${kind} · синхронизация…${mrError}` };
  if (!tracker.ok) return { ok: false, text: `${kind} недоступен: ${failure(tracker.error)}${mrError}`, at: tracker.at };
  return { ok: !mrError, text: `${kind} · синхр. ${ago(tracker.at)}${mrError}`, at: tracker.at };
});

async function syncNow() {
  const id = state.value.workspaceId ?? workspace.value?.id;
  if (!id || syncing.value) return;
  syncing.value = true;
  await client.syncWorkspace(id).catch(() => undefined);
  syncing.value = false;
}
</script>

<template>
  <div class="tasks">
    <div class="sticky">
      <div v-if="syncLine" class="sync" :class="{ failed: !syncLine.ok }" data-testid="sync-status" :title="syncLine.at ? new Date(syncLine.at).toLocaleString() : undefined">
        <span>{{ syncLine.text }}</span>
        <VButton label="Обновить" size="small" text :loading="syncing" data-testid="sync-now" @click="syncNow" />
      </div>
      <div class="filters">
        <VMultiSelect
          v-model="filters.releases"
          :options="releaseOptions"
          option-label="label"
          option-value="value"
          placeholder="Релиз"
          filter
          :max-selected-labels="2"
          selected-items-label="Выбрано: {0}"
          size="small"
          data-testid="filter-release"
        />
        <VMultiSelect
          v-model="filters.statuses"
          :options="statusOptions"
          placeholder="Статус"
          filter
          :max-selected-labels="2"
          selected-items-label="Выбрано: {0}"
          size="small"
          data-testid="filter-status"
        />
        <VMultiSelect
          v-model="filters.priorities"
          :options="priorityOptions"
          option-label="label"
          option-value="value"
          placeholder="Приоритет"
          :max-selected-labels="2"
          selected-items-label="Выбрано: {0}"
          size="small"
          data-testid="filter-priority"
        />
        <VMultiSelect
          v-model="filters.labels"
          :options="labelOptions"
          placeholder="Метки"
          filter
          :max-selected-labels="2"
          selected-items-label="Выбрано: {0}"
          size="small"
          data-testid="filter-labels"
        />
        <VSelect v-model="filters.assignee" :options="assigneeOptions" option-label="label" option-value="value" filter size="small" data-testid="filter-assignee" />
        <VInputText v-model="filters.text" placeholder="Поиск" size="small" data-testid="filter-text" />
        <label class="toggle">
          <VToggleSwitch v-model="filters.hideClosed" data-testid="filter-hide-closed" />
          Скрыть закрытые
        </label>
        <VButton label="Сбросить" size="small" text data-testid="filter-reset" @click="reset" />
        <span class="vr-muted count" data-testid="tasks-count">{{ visible.length }} из {{ state.tasks.length }} задач</span>
      </div>
    </div>
    <section v-for="group in groups" :key="group.id">
      <h3>{{ group.name }}</h3>
      <button
        v-for="task in group.tasks"
        :key="task.id"
        class="task"
        :class="{ selected: task.id === selectedId }"
        :aria-current="task.id === selectedId ? 'true' : undefined"
        :data-testid="`task-${task.externalRef}`"
        @click="open(task)"
      >
        <span class="ref">{{ task.externalRef }}</span>
        <span class="title" :title="task.title">{{ task.title }}</span>
        <span class="vr-muted meta">
          <TaskMeta :task="task" />
          <span v-if="mrCount(task)" class="mr-count" :title="`Merge requests: ${mrCount(task)}`" data-testid="task-mr-count">MR {{ mrCount(task) }}</span>
          {{ assigneeLabel(task) }} · {{ task.status }}
          <span v-for="id in task.delegateIds" :key="id" class="vr-dot" :data-status="agentStatus(id)" :title="client.actorName(id)" />
        </span>
      </button>
    </section>
    <div v-if="!visible.length" class="empty" data-testid="tasks-empty">
      <template v-if="state.tasks.length">
        <span>Нет задач по фильтрам</span>
        <VButton label="Сбросить" size="small" text data-testid="empty-reset" @click="reset" />
      </template>
      <span v-else class="vr-muted">Задач пока нет</span>
    </div>
  </div>
</template>

<style scoped>
.tasks {
  height: 100%;
  overflow: auto;
  padding: var(--vr-space-3);
  background: var(--vr-color-surface);
  color: var(--vr-color-text);
  box-sizing: border-box;
}

.sticky {
  position: sticky;
  top: calc(-1 * var(--vr-space-3));
  z-index: 1;
  background: var(--vr-color-surface);
}

.sync {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: 12px;
  color: var(--vr-color-text-muted);
}

.sync.failed {
  color: var(--vr-color-danger);
}

.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) 0;
  background: var(--vr-color-surface);
  border-bottom: 1px solid var(--vr-color-border);
}

.toggle {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  font-size: 12px;
}

.count {
  margin-left: auto;
  font-size: 12px;
}

h3 {
  margin: var(--vr-space-2) 0;
  font-size: 13px;
  text-transform: uppercase;
  color: var(--vr-color-text-muted);
}

.task {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 2px var(--vr-space-2);
  width: 100%;
  padding: var(--vr-space-2);
  margin-bottom: var(--vr-space-1);
  text-align: left;
  font: inherit;
  color: inherit;
  background: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.task:hover {
  border-color: var(--vr-color-accent);
}

.task.selected {
  border-color: var(--vr-color-accent);
  background: var(--vr-color-surface-muted);
  box-shadow: inset 3px 0 0 var(--vr-color-accent);
}

.title {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  overflow: hidden;
}

.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-4);
}

.ref {
  font-family: var(--vr-font-mono);
  color: var(--vr-color-accent);
}

.mr-count {
  padding: 0 6px;
  font-size: 11px;
  line-height: 18px;
  border-radius: 9px;
  color: var(--vr-color-accent);
  border: 1px solid var(--vr-color-accent);
}

.meta {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  font-size: 12px;
}
</style>
