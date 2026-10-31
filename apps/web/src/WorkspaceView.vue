<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import type { WorkspaceInfo } from '@core/client-sdk';
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
const workspaces = ref<WorkspaceInfo[]>([]);
const WORKSPACE_KEY = 'veyra.workspace';

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
  await client.setWorkspace(id);
  for (const panel of [...(dock?.panels ?? [])]) if (panel.id.startsWith('task:')) dock?.removePanel(panel);
}

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

let dock: DockviewApi | null = null;

function openAgents() {
  if (!dock) return;
  const existing = dock.getPanel('agents');
  if (existing) return existing.api.setActive();
  const position = dock.getPanel('tasks') ? { direction: 'below' as const, referencePanel: 'tasks' } : undefined;
  dock.addPanel({ id: 'agents', component: 'agents', title: 'Мои агенты', position });
}

function onReady({ api }: DockviewReadyEvent) {
  dock = api;
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
      <select
        v-if="workspaces.length > 1"
        class="workspace-switch"
        data-testid="workspace-switch"
        aria-label="Компания"
        :value="state.workspaceId ?? workspaces[0]?.id"
        @change="switchWorkspace(($event.target as HTMLSelectElement).value)"
      >
        <option v-for="w in workspaces" :key="w.id" :value="w.id">{{ w.name }}</option>
      </select>
      <span class="spacer" />
      <VButton size="small" text label="Мои агенты" data-testid="open-agents" @click="openAgents" />
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

.workspace-switch {
  font: inherit;
  color: inherit;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: 4px;
  padding: 2px var(--vr-space-2);
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
