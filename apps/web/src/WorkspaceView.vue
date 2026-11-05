<script setup lang="ts">
import { watch } from 'vue';
import { useClient, useClientState } from '@core/vue-bindings';
import AgentsView from './agents/AgentsView.vue';
import Sidebar from './Sidebar.vue';
import { view, type View } from './nav';
import { isDark, toggleTheme } from './settings/appearance';
import SettingsView from './settings/SettingsView.vue';
import ConfluenceView from './views/ConfluenceView.vue';
import JiraView from './views/JiraView.vue';
import ReviewsView from './views/ReviewsView.vue';
import TasksView from './views/TasksView.vue';
import TaskView from './views/TaskView.vue';
import TopicView from './views/TopicView.vue';

const client = useClient();
const state = useClientState();
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
  if (view.value.kind !== 'tasks') view.value = { kind: 'tasks' };
  client.setWorkspace(id);
}

</script>

<template>
  <div class="shell">
    <Sidebar :dark="isDark" @switch="switchWorkspace" @theme="toggleTheme" />
    <main class="card">
      <KeepAlive include="AgentsView">
        <AgentsView v-if="view.kind === 'agents'" />
        <ReviewsView v-else-if="view.kind === 'reviews'" />
        <JiraView v-else-if="view.kind === 'jira'" />
        <ConfluenceView v-else-if="view.kind === 'confluence'" />
        <SettingsView v-else-if="view.kind === 'settings'" />
        <TaskView v-else-if="view.kind === 'task'" :key="view.id" :task-id="view.id" />
        <TopicView v-else-if="view.kind === 'topic'" :key="view.id" :topic-id="view.id" />
        <TasksView v-else />
      </KeepAlive>
    </main>
  </div>
</template>

<style scoped>
.shell {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  height: 100%;
  min-width: 0;
  overflow: hidden;
}

.card {
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
  margin: var(--vr-space-2) var(--vr-space-2) var(--vr-space-2) 1px;
  overflow: hidden;
  background: var(--vr-color-surface);
  border-radius: var(--vr-radius-2xl);
  box-shadow: var(--vr-elevation-card);
}
</style>
