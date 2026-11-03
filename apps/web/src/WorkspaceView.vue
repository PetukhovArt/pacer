<script setup lang="ts">
import { ref, watch } from 'vue';
import { useClient, useClientState } from '@core/vue-bindings';
import { DARK_CLASS } from '@ui/kit';
import Sidebar from './Sidebar.vue';
import { view, type View } from './nav';
import TasksView from './views/TasksView.vue';
import TaskView from './views/TaskView.vue';

const client = useClient();
const state = useClientState();
const dark = ref(document.documentElement.classList.contains(DARK_CLASS));
const WORKSPACE_KEY = 'veyra.workspace';
const viewKey = `veyra.view.${state.value.me.id}`;

try {
  view.value = JSON.parse(localStorage.getItem(viewKey) ?? '') as View;
} catch {
  view.value = { kind: 'tasks' };
}
watch(view, (value) => localStorage.setItem(viewKey, JSON.stringify(value)));

function switchWorkspace(id: string) {
  localStorage.setItem(WORKSPACE_KEY, id);
  if (view.value.kind === 'task') view.value = { kind: 'tasks' };
  client.setWorkspace(id);
}

function toggleTheme() {
  dark.value = !dark.value;
  document.documentElement.classList.toggle(DARK_CLASS, dark.value);
}
</script>

<template>
  <div class="shell">
    <Sidebar :dark="dark" @switch="switchWorkspace" @theme="toggleTheme" />
    <main class="card">
      <TaskView v-if="view.kind === 'task'" :key="view.id" :task-id="view.id" />
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
</style>
