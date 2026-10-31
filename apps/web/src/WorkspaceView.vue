<script setup lang="ts">
import { ref } from 'vue';
import { DockviewVue } from 'dockview-vue';
import { themeAbyss, themeLight, type DockviewApi, type DockviewReadyEvent } from 'dockview-core';
import { useClient, useClientState } from '@core/vue-bindings';
import { DARK_CLASS, VButton } from '@ui/kit';
import { panels } from './panels';

defineOptions({ components: panels });

const client = useClient();
const state = useClientState();
const dark = ref(document.documentElement.classList.contains(DARK_CLASS));
const layoutKey = `veyra.layout.${state.value.me?.id}`;
const runnerCommand = ref('');

async function connectRunner() {
  runnerCommand.value = (await client.runnerToken()).command;
}

function toggleTheme() {
  dark.value = !dark.value;
  document.documentElement.classList.toggle(DARK_CLASS, dark.value);
}

function defaultLayout(api: DockviewApi) {
  api.addPanel({ id: 'tasks', component: 'tasks', title: 'Задачи' });
}

function onReady({ api }: DockviewReadyEvent) {
  const saved = localStorage.getItem(layoutKey);
  try {
    if (saved) api.fromJSON(JSON.parse(saved));
  } catch {
    api.clear();
  }
  if (api.panels.length === 0) defaultLayout(api);
  api.onDidLayoutChange(() => localStorage.setItem(layoutKey, JSON.stringify(api.toJSON())));
}
</script>

<template>
  <div class="workspace">
    <header>
      <strong>Veyra</strong>
      <span class="vr-muted">{{ state.me?.name }}</span>
      <span class="vr-dot" :data-online="state.connected" :title="state.connected ? 'online' : 'offline'" />
      <span class="spacer" />
      <VButton size="small" text :label="dark ? 'Светлая' : 'Тёмная'" @click="toggleTheme" />
      <VButton size="small" text label="Подключить runner" data-testid="connect-runner" @click="connectRunner" />
      <VButton size="small" text label="Выйти" data-testid="logout" @click="client.logout()" />
    </header>
    <div v-if="runnerCommand" class="runner-command" data-testid="runner-command">
      <span class="vr-muted">Запустите в репозитории:</span>
      <code>{{ runnerCommand }}</code>
      <VButton size="small" text label="Скрыть" @click="runnerCommand = ''" />
    </div>
    <div class="dock">
      <div class="dock-view">
        <DockviewVue :theme="dark ? themeAbyss : themeLight" class="dock-fill" @ready="onReady" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.workspace {
  display: flex;
  flex-direction: column;
  height: 100%;
}

header {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-2) var(--vr-space-4);
  background: var(--vr-color-surface);
  border-bottom: 1px solid var(--vr-color-border);
}

.runner-command {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-2) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.runner-command code {
  user-select: all;
  overflow-wrap: anywhere;
}

.spacer {
  flex: 1;
}

.dock {
  flex: 1;
  min-height: 0;
  position: relative;
}

.dock-view {
  position: absolute;
  inset: 0;
}

.dock-view > :deep(.dock-fill) {
  height: 100%;
}
</style>
