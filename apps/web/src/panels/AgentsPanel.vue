<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import type { IDockviewPanelProps } from 'dockview-core';
import type { AgentStatus, AgentSummary } from '@veyra/protocol';
import { useClient, useClientState } from '@core/vue-bindings';
import { openTask } from '../open-task';

const props = defineProps<{ params: IDockviewPanelProps }>();
const client = useClient();
const state = useClientState();
const items = ref<AgentSummary[]>([]);
const error = ref('');

const ORDER: Record<AgentStatus, number> = { running: 0, queued: 1, failed: 2, done: 3, stopped: 4 };

async function load() {
  try {
    items.value = await client.myAgents();
    error.value = '';
  } catch (e) {
    error.value = (e as Error).message;
  }
}

let timer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => [state.value.actors, state.value.workspaceId],
  () => {
    clearTimeout(timer);
    timer = setTimeout(() => void load(), 800);
  },
);
onMounted(load);
onUnmounted(() => clearTimeout(timer));

const rows = computed(() =>
  items.value
    .map((item) => {
      const live = state.value.actors.get(item.agent.id);
      const agent = live?.kind === 'agent' ? live : item.agent;
      return { ...item, agent, task: state.value.tasks.find((t) => t.id === agent.taskId) };
    })
    .sort((a, b) => ORDER[a.agent.status] - ORDER[b.agent.status] || (b.startedAt ?? '').localeCompare(a.startedAt ?? '')),
);

function projectName(id: string | null | undefined) {
  if (!id) return '';
  return state.value.workspaces.find((w) => w.id === state.value.workspaceId)?.projects?.find((p) => p.id === id)?.name ?? id;
}

function open(row: (typeof rows.value)[number]) {
  if (row.task) openTask(props.params.containerApi, row.task);
}
</script>

<template>
  <div class="agents" data-testid="agents-panel">
    <p v-if="error" class="error">{{ error }}</p>
    <p v-else-if="!rows.length" class="vr-muted">У вас нет агентов в этой компании</p>
    <button
      v-for="row in rows"
      :key="row.agent.id"
      class="agent"
      :data-testid="`agent-row-${row.agent.id.slice(0, 8)}`"
      :disabled="!row.task"
      @click="open(row)"
    >
      <span class="vr-dot" :data-status="row.agent.status" :title="row.agent.status" />
      <span class="ref">{{ row.task?.externalRef ?? row.agent.taskId ?? '—' }}</span>
      <span class="title">{{ row.task?.title ?? '' }}</span>
      <span class="vr-muted meta">
        {{ row.agent.status }}
        <template v-if="row.agent.projectId"> · {{ projectName(row.agent.projectId) }}</template>
        · ${{ row.costUsd.toFixed(2) }}
        <template v-if="row.startedAt"> · {{ new Date(row.startedAt).toLocaleString() }}</template>
      </span>
    </button>
  </div>
</template>

<style scoped>
.agents {
  height: 100%;
  overflow: auto;
  padding: var(--vr-space-3);
  box-sizing: border-box;
  background: var(--vr-color-surface);
  color: var(--vr-color-text);
}

.agent {
  display: grid;
  grid-template-columns: auto auto 1fr;
  align-items: center;
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

.agent:hover:not(:disabled) {
  border-color: var(--vr-color-accent);
}

.ref {
  font-family: var(--vr-font-mono);
  color: var(--vr-color-accent);
}

.title {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.meta {
  grid-column: 2 / -1;
  font-size: 12px;
}

.error {
  color: var(--vr-color-danger);
}
</style>
