<script setup lang="ts">
import { computed, reactive, watch } from 'vue';
import type { Task } from '@veyra/protocol';
import { VButton, VInputText, VMultiSelect, VSelect, VToggleSwitch } from '@ui/kit';
import { useClient, useClientState } from '@core/vue-bindings';
import { openTask } from '../nav';
import { priorityRank } from '../priority';
import TaskMeta from './TaskMeta.vue';

type Filters = { releases: string[]; statuses: string[]; priorities: string[]; labels: string[]; assignee: string; text: string; hideClosed: boolean };

const NO_RELEASE = 'none';
const NO_PRIORITY = 'none';
const defaults = (): Filters => ({ releases: [], statuses: [], priorities: [], labels: [], assignee: 'all', text: '', hideClosed: true });

const client = useClient();
const state = useClientState();
const storageKey = () => `veyra.taskFilters.${state.value.me.id}.${state.value.workspaceId ?? ''}`;

function load(): Filters {
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

const assigneeOptions = computed(() => {
  const ids = new Set(state.value.tasks.flatMap((t) => (t.assigneeId ? [t.assigneeId] : [])));
  const all = [...ids].map((id) => ({ label: client.actorName(id), value: `actor:${id}` })).sort((a, b) => a.label.localeCompare(b.label));
  return [{ label: 'Все', value: 'all' }, { label: 'Мои', value: 'mine' }, { label: 'Не назначены', value: 'unassigned' }, ...all];
});

function isMine(task: Task) {
  return task.assigneeId === state.value.me.id;
}

function assigneeLabel(task: Task) {
  return task.assigneeId ? client.actorName(task.assigneeId) : 'не назначена';
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
  if (filters.assignee === 'mine' && !isMine(task)) return false;
  if (filters.assignee === 'unassigned' && task.assigneeId) return false;
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
  return mrCounts.value.get(task.id) ?? 0;
}
</script>

<template>
  <div class="page">
    <header>
      <h1>Задачи</h1>
      <span class="vr-muted" data-testid="tasks-count">{{ visible.length }} из {{ state.tasks.length }}</span>
    </header>
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
    </div>
    <div class="body">
      <section v-for="group in groups" :key="group.id">
        <h2>{{ group.name }}</h2>
        <ul class="vr-list">
          <li v-for="task in group.tasks" :key="task.id">
            <button class="vr-row" :data-testid="`task-${task.externalRef}`" @click="openTask(task.id)">
              <span class="line">
                <strong class="ref">{{ task.externalRef }}</strong>
                <span class="title" :title="task.title">{{ task.title }}</span>
              </span>
              <span class="vr-row-meta">
                <TaskMeta :task="task" />
                <span v-if="mrCount(task)" class="vr-chip" :title="`Merge requests: ${mrCount(task)}`" data-testid="task-mr-count">MR {{ mrCount(task) }}</span>
                <span>{{ assigneeLabel(task) }}</span>
                <span>{{ task.status }}</span>
              </span>
            </button>
          </li>
        </ul>
      </section>
      <div v-if="!visible.length" class="empty" data-testid="tasks-empty">
        <template v-if="state.tasks.length">
          <span>Нет задач по фильтрам</span>
          <VButton label="Сбросить" size="small" text data-testid="empty-reset" @click="reset" />
        </template>
        <span v-else class="vr-muted">Задач пока нет</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

header {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  padding: var(--vr-space-4) var(--vr-space-5) var(--vr-space-2);
}

h1 {
  margin: 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
  letter-spacing: -0.01em;
}

.filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-5) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.toggle {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
}

.body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-2) var(--vr-space-5) var(--vr-space-5);
}

h2 {
  margin: var(--vr-space-4) 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
}

.line {
  display: flex;
  gap: var(--vr-space-2);
  min-width: 0;
}

.ref {
  flex: none;
  font-weight: 600;
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
  padding: var(--vr-space-6);
}
</style>
