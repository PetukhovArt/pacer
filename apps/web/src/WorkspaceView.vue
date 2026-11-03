<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import type { WorkspaceInfo } from '@core/client-sdk';
import { X } from 'lucide-vue-next';
import { useClient, useClientState } from '@core/vue-bindings';
import { DARK_CLASS } from '@ui/kit';
import Sidebar from './Sidebar.vue';
import { view, type View } from './nav';
import AgentsView from './views/AgentsView.vue';
import TasksView from './views/TasksView.vue';
import TaskView from './views/TaskView.vue';

const client = useClient();
const state = useClientState();
const dark = ref(document.documentElement.classList.contains(DARK_CLASS));
const runnerCommand = ref('');
const workspaces = ref<WorkspaceInfo[]>([]);
const WORKSPACE_KEY = 'veyra.workspace';
const viewKey = `veyra.view.${state.value.me?.id}`;

try {
  view.value = JSON.parse(localStorage.getItem(viewKey) ?? '') as View;
} catch {
  view.value = { kind: 'tasks' };
}
watch(view, (value) => localStorage.setItem(viewKey, JSON.stringify(value)));

const lastStatus = new Map<string, string>();

watch(
  () => state.value.actors,
  (actors) => {
    for (const actor of actors.values()) {
      if (actor.kind !== 'agent' || actor.ownerId !== state.value.me?.id) continue;
      const previous = lastStatus.get(actor.id);
      lastStatus.set(actor.id, actor.status);
      if (!previous || previous === actor.status || !['done', 'failed'].includes(actor.status)) continue;
      if (document.hasFocus() || typeof Notification === 'undefined' || Notification.permission !== 'granted') continue;
      const task = state.value.tasks.find((t) => t.id === actor.taskId);
      const title = actor.status === 'done' ? 'Агент закончил' : 'Агент упал';
      new Notification(title, { body: task ? `${task.externalRef} · ${task.title}` : actor.name, tag: actor.id });
    }
  },
  { immediate: true },
);

onMounted(async () => {
  if (typeof Notification !== 'undefined' && Notification.permission === 'default') void Notification.requestPermission().catch(() => undefined);
  workspaces.value = await client.workspaces().catch(() => []);
  const current = state.value.workspaceId;
  if (workspaces.value.length && !workspaces.value.some((w) => w.id === current)) await switchWorkspace(workspaces.value[0]!.id);
});

async function switchWorkspace(id: string) {
  localStorage.setItem(WORKSPACE_KEY, id);
  if (view.value.kind === 'task') view.value = { kind: 'tasks' };
  await client.setWorkspace(id);
}

async function connectRunner() {
  runnerCommand.value = (await client.runnerToken()).command;
}

function toggleTheme() {
  dark.value = !dark.value;
  document.documentElement.classList.toggle(DARK_CLASS, dark.value);
}
</script>

<template>
  <div class="shell">
    <Sidebar :workspaces="workspaces" :dark="dark" @switch="switchWorkspace" @theme="toggleTheme" @runner="connectRunner" />
    <main class="card">
      <div v-if="runnerCommand" class="runner-command" data-testid="runner-command">
        <span class="vr-muted">Запустите в репозитории:</span>
        <code class="vr-code">{{ runnerCommand }}</code>
        <button class="close" aria-label="Скрыть" title="Скрыть" @click="runnerCommand = ''"><X :size="14" /></button>
      </div>
      <TaskView v-if="view.kind === 'task'" :key="view.id" :task-id="view.id" />
      <AgentsView v-else-if="view.kind === 'agents'" />
      <TasksView v-else />
    </main>
  </div>
</template>

<style scoped>
.shell {
  display: grid;
  grid-template-columns: var(--vr-size-sidebar) minmax(0, 1fr);
  height: 100%;
}

.card {
  display: flex;
  flex-direction: column;
  min-height: 0;
  margin: var(--vr-space-2) var(--vr-space-2) var(--vr-space-2) 1px;
  overflow: hidden;
  background: var(--vr-color-surface);
  border-radius: var(--vr-radius-2xl);
  box-shadow: var(--vr-elevation-card);
}

.runner-command {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-2) var(--vr-space-5);
  font-size: var(--vr-font-size-xs);
  border-bottom: 1px solid var(--vr-color-border);
}

.runner-command code {
  flex: 1;
  user-select: all;
}

.close {
  display: inline-flex;
  padding: var(--vr-space-1);
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}
</style>
