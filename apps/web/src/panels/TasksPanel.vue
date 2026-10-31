<script setup lang="ts">
import { computed } from 'vue';
import type { IDockviewPanelProps } from 'dockview-core';
import type { Task } from '@veyra/protocol';
import { useClient, useClientState } from '@core/vue-bindings';
import { openTask } from '../open-task';

const props = defineProps<{ params: IDockviewPanelProps }>();
const client = useClient();
const state = useClientState();

const groups = computed(() => {
  const { releases, tasks } = state.value;
  const byRelease = releases.map((r) => ({ id: r.id, name: r.name, tasks: tasks.filter((t) => t.releaseId === r.id) }));
  return [...byRelease, { id: 'none', name: 'Без релиза', tasks: tasks.filter((t) => !t.releaseId) }].filter((g) => g.tasks.length);
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
    <section v-for="group in groups" :key="group.id">
      <h3>{{ group.name }}</h3>
      <button v-for="task in group.tasks" :key="task.id" class="task" :data-testid="`task-${task.externalRef}`" @click="open(task)">
        <span class="ref">{{ task.externalRef }}</span>
        <span class="title">{{ task.title }}</span>
        <span class="vr-muted meta">
          {{ task.assigneeId ? client.actorName(task.assigneeId) : 'не назначена' }} · {{ task.status }}
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
