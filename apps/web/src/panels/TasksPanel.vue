<script setup lang="ts">
import { computed, reactive, watch } from 'vue';
import type { IDockviewPanelProps } from 'dockview-core';
import type { Task } from '@veyra/protocol';
import { VButton, VInputText, VMultiSelect, VSelect, VToggleSwitch } from '@ui/kit';
import { useClient, useClientState } from '@core/vue-bindings';
import { openTask } from '../open-task';

type Filters = { releases: string[]; statuses: string[]; assignee: string; text: string; hideClosed: boolean };

const NO_RELEASE = 'none';
const defaults = (): Filters => ({ releases: [], statuses: [], assignee: 'all', text: '', hideClosed: true });

const props = defineProps<{ params: IDockviewPanelProps }>();
const client = useClient();
const state = useClientState();
const storageKey = `veyra.taskFilters.${state.value.me?.id}`;

function load(): Filters {
  const fromPanel = (props.params.params as { filters?: Partial<Filters> } | undefined)?.filters;
  if (fromPanel) return { ...defaults(), ...fromPanel };
  try {
    return { ...defaults(), ...(JSON.parse(localStorage.getItem(storageKey) ?? '{}') as Partial<Filters>) };
  } catch {
    return defaults();
  }
}

const filters = reactive<Filters>(load());

watch(
  filters,
  (value) => {
    const snapshot = { ...value, releases: [...value.releases], statuses: [...value.statuses] };
    localStorage.setItem(storageKey, JSON.stringify(snapshot));
    props.params.api.updateParameters({ filters: snapshot });
  },
  { deep: true },
);

function reset() {
  Object.assign(filters, defaults());
}

const sortedReleases = computed(() => [...state.value.releases].sort((a, b) => b.name.localeCompare(a.name, undefined, { numeric: true })));

const releaseOptions = computed(() => [...sortedReleases.value.map((r) => ({ label: r.name, value: r.id })), { label: 'Без релиза', value: NO_RELEASE }]);

const statusOptions = computed(() => [...new Set(state.value.tasks.map((t) => t.status))].sort((a, b) => a.localeCompare(b)));

const trackerUser = computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId)?.trackerUser ?? null);

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
  if (filters.assignee === 'mine' && !isMine(task)) return false;
  if (filters.assignee === 'unassigned' && (task.assigneeId || task.assigneeRef)) return false;
  if (filters.assignee.startsWith('ref:') && task.assigneeRef !== filters.assignee.slice(4)) return false;
  if (filters.assignee.startsWith('actor:') && task.assigneeId !== filters.assignee.slice(6)) return false;
  const text = filters.text.trim().toLowerCase();
  return !text || task.externalRef.toLowerCase().includes(text) || task.title.toLowerCase().includes(text);
}

const visible = computed(() => state.value.tasks.filter(matches));

const groups = computed(() => {
  const tasks = visible.value;
  const byRelease = sortedReleases.value.map((r) => ({ id: r.id, name: r.name, tasks: tasks.filter((t) => t.releaseId === r.id) }));
  const known = new Set(sortedReleases.value.map((r) => r.id));
  const none = { id: NO_RELEASE, name: 'Без релиза', tasks: tasks.filter((t) => !t.releaseId || !known.has(t.releaseId)) };
  return [...byRelease, none].filter((g) => g.tasks.length);
});

function agentStatus(id: string) {
  const actor = state.value.actors.get(id);
  return actor?.kind === 'agent' ? actor.status : null;
}

function open(task: Task) {
  openTask(props.params.containerApi, task);
}
</script>

<template>
  <div class="tasks">
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
      <VSelect v-model="filters.assignee" :options="assigneeOptions" option-label="label" option-value="value" filter size="small" data-testid="filter-assignee" />
      <VInputText v-model="filters.text" placeholder="Поиск" size="small" data-testid="filter-text" />
      <label class="toggle">
        <VToggleSwitch v-model="filters.hideClosed" data-testid="filter-hide-closed" />
        Скрыть закрытые
      </label>
      <VButton label="Сбросить" size="small" text data-testid="filter-reset" @click="reset" />
      <span class="vr-muted count" data-testid="tasks-count">{{ visible.length }} из {{ state.tasks.length }} задач</span>
    </div>
    <section v-for="group in groups" :key="group.id">
      <h3>{{ group.name }}</h3>
      <button v-for="task in group.tasks" :key="task.id" class="task" :data-testid="`task-${task.externalRef}`" @click="open(task)">
        <span class="ref">{{ task.externalRef }}</span>
        <span class="title">{{ task.title }}</span>
        <span class="vr-muted meta">
          {{ assigneeLabel(task) }} · {{ task.status }}
          <span v-for="id in task.delegateIds" :key="id" class="vr-dot" :data-status="agentStatus(id)" :title="client.actorName(id)" />
        </span>
      </button>
    </section>
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

.filters {
  position: sticky;
  top: calc(-1 * var(--vr-space-3));
  z-index: 1;
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

.ref {
  font-family: var(--vr-font-mono);
  color: var(--vr-color-accent);
}

.meta {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  font-size: 12px;
}
</style>
