<script setup lang="ts">
import { watch } from 'vue';
import { useClient, useClientState } from '@core/vue-bindings';
import AgentsView from './agents/AgentsView.vue';
import CommandPalette from './commands/CommandPalette.vue';
import { paletteOpen } from './commands/context';
import MobileNav from './mobile/MobileNav.vue';
import { useIsMobile } from './mobile/useIsMobile';
import Sidebar from './Sidebar.vue';
import TitleBar from './TitleBar.vue';
import { installTray } from './tray';
import { navigate, view, type View } from './nav';
import { isDark, toggleTheme } from './settings/appearance';
import SettingsView from './settings/SettingsView.vue';
import ConfluenceView from './views/ConfluenceView.vue';
import JiraView from './views/JiraView.vue';
import TasksView from './views/TasksView.vue';
import TaskView from './views/TaskView.vue';
import TopicView from './views/TopicView.vue';
import { useDock } from './workbench/dock';
import QuickOpen from './workbench/QuickOpen.vue';

const inTauri = '__TAURI_INTERNALS__' in window;
const mobile = useIsMobile();
const client = useClient();
const state = useClientState();
const WORKSPACE_KEY = 'veyra.workspace';
const viewKey = `veyra.view.v2.${state.value.me.id}`;
const dock = useDock();
installTray();

try {
  view.value = JSON.parse(localStorage.getItem(viewKey) ?? '') as View;
} catch {
  view.value = { kind: 'agents' };
}
watch(
  view,
  (value) => {
    if (value.kind === 'reviews') {
      view.value = { kind: 'agents' };
      if (value.open) dock.openReview(value.open);
      return;
    }
    localStorage.setItem(viewKey, JSON.stringify(value));
  },
  { immediate: true },
);

function switchWorkspace(id: string) {
  localStorage.setItem(WORKSPACE_KEY, id);
  if (view.value.kind === 'task' || view.value.kind === 'topic') void navigate({ kind: 'agents' });
  client.setWorkspace(id);
}
</script>

<template>
  <div class="shell">
    <TitleBar v-if="inTauri" />
    <div class="body" :data-mobile="mobile">
      <Sidebar v-if="!mobile" :dark="isDark" @switch="switchWorkspace" @theme="toggleTheme" />
      <main class="card vt-main">
        <KeepAlive include="AgentsView">
          <AgentsView v-if="view.kind === 'agents'" />
          <JiraView v-else-if="view.kind === 'jira'" />
          <ConfluenceView v-else-if="view.kind === 'confluence'" />
          <SettingsView v-else-if="view.kind === 'settings'" />
          <TaskView v-else-if="view.kind === 'task'" :key="view.id" :task-id="view.id" />
          <TopicView v-else-if="view.kind === 'topic'" :key="view.id" :topic-id="view.id" />
          <TasksView v-else />
        </KeepAlive>
      </main>
    </div>
    <MobileNav v-if="mobile" />
    <QuickOpen />
    <Transition name="palette">
      <CommandPalette v-if="paletteOpen" />
    </Transition>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  overflow: hidden;
}

.body {
  display: grid;
  flex: 1;
  grid-template-columns: auto minmax(0, 1fr);
  min-height: 0;
  min-width: 0;
}

.body[data-mobile='true'] {
  grid-template-columns: minmax(0, 1fr);
}

.body[data-mobile='true'] .card {
  margin: 0;
  border-radius: 0;
  box-shadow: none;
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
