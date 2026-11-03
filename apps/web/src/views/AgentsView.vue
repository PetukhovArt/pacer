<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import type { AgentStatus, AgentSummary } from '@veyra/protocol';
import { useClient, useClientState } from '@core/vue-bindings';
import Avatar from '../Avatar.vue';
import { openTask } from '../nav';

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
</script>

<template>
  <div class="page" data-testid="agents-panel">
    <header>
      <h1>Мои агенты</h1>
      <span class="vr-muted">{{ rows.length }}</span>
    </header>
    <div class="body">
      <p v-if="error" class="error">{{ error }}</p>
      <p v-else-if="!rows.length" class="vr-muted">У вас нет агентов в этой компании. Запустите агента из задачи.</p>
      <ul v-else class="vr-list">
        <li v-for="row in rows" :key="row.agent.id">
          <button class="vr-row agent" :data-testid="`agent-row-${row.agent.id.slice(0, 8)}`" :disabled="!row.task" @click="row.task && openTask(row.task.id)">
            <span class="line">
              <Avatar :id="row.agent.id" :name="row.agent.name" agent small />
              <strong>{{ row.task?.externalRef ?? row.agent.taskId ?? '—' }}</strong>
              <span class="title">{{ row.task?.title ?? row.agent.name }}</span>
            </span>
            <span class="vr-row-meta">
              <span class="vr-dot" :data-status="row.agent.status" />
              {{ row.agent.status }}
              <span v-if="row.agent.projectId">{{ projectName(row.agent.projectId) }}</span>
              <span>${{ row.costUsd.toFixed(2) }}</span>
              <span v-if="row.startedAt">{{ new Date(row.startedAt).toLocaleString() }}</span>
            </span>
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

header {
  display: flex;
  align-items: baseline;
  gap: var(--vr-space-2);
  padding: var(--vr-space-4) var(--vr-space-5) var(--vr-space-3);
}

h1 {
  margin: 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
  letter-spacing: -0.01em;
}

.body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 var(--vr-space-5) var(--vr-space-5);
}

.line {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.title {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.agent .vr-row-meta {
  padding-left: calc(var(--vr-size-avatar-sm) + var(--vr-space-2));
}

.error {
  color: var(--vr-color-danger);
}
</style>
