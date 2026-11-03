<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import type { WorkspaceInfo } from '@core/client-sdk';
import type { Task } from '@veyra/protocol';
import { Bot, ListTodo, LogOut, Moon, Plug, Search, Sun } from 'lucide-vue-next';
import { useClient, useClientState } from '@core/vue-bindings';
import Avatar from './Avatar.vue';
import { openTask, view } from './nav';
import { priorityRank } from './priority';

defineProps<{ workspaces: WorkspaceInfo[]; dark: boolean }>();
const emit = defineEmits<{ switch: [id: string]; theme: []; runner: [] }>();

const client = useClient();
const state = useClientState();
const query = ref('');
const searchInput = ref<HTMLInputElement | null>(null);

const selectedId = computed(() => (view.value.kind === 'task' ? view.value.id : null));
const workspace = computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId));

function isMine(task: Task) {
  const trackerUser = workspace.value?.trackerUser;
  return task.assigneeId === state.value.me?.id || (!!trackerUser && task.assigneeRef === trackerUser);
}

const mine = computed(() => state.value.tasks.filter((t) => isMine(t) && t.statusCategory !== 'done'));

const channels = computed(() => {
  const text = query.value.trim().toLowerCase();
  const list = text
    ? state.value.tasks.filter((t) => t.externalRef.toLowerCase().includes(text) || t.title.toLowerCase().includes(text)).slice(0, 50)
    : [...mine.value, ...state.value.tasks.filter((t) => t.id === selectedId.value && !mine.value.includes(t))];
  return [...list].sort((a, b) => priorityRank(a.priority) - priorityRank(b.priority) || a.externalRef.localeCompare(b.externalRef, undefined, { numeric: true }));
});

const unread = ref(new Set<string>());
const seen = new Map<string, number>();

watch(
  () => state.value.events,
  (events) => {
    const next = new Set(unread.value);
    for (const [taskId, list] of events) {
      const before = seen.get(taskId);
      seen.set(taskId, list.length);
      if (before !== undefined && list.length > before && taskId !== selectedId.value) next.add(taskId);
    }
    if (next.size !== unread.value.size) unread.value = next;
  },
);

watch(selectedId, (id) => {
  if (!id || !unread.value.has(id)) return;
  const next = new Set(unread.value);
  next.delete(id);
  unread.value = next;
});

const loaded = new Set<string>();
watch(
  mine,
  (tasks) => {
    for (const task of tasks) {
      if (loaded.has(task.id) || state.value.fields.has(task.id)) continue;
      loaded.add(task.id);
      void client.loadTask(task.id).catch(() => loaded.delete(task.id));
    }
  },
  { immediate: true },
);
watch(
  () => state.value.workspaceId,
  () => loaded.clear(),
);

function working(task: Task) {
  return task.delegateIds.some((id) => {
    const actor = state.value.actors.get(id);
    return actor?.kind === 'agent' && actor.status === 'running';
  });
}

function onSearchKey(event: KeyboardEvent) {
  if (event.key === 'Escape') query.value = '';
  if (event.key === 'Enter' && channels.value[0]) openTask(channels.value[0].id);
}

function onGlobalKey(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    searchInput.value?.focus();
  }
}

onMounted(() => window.addEventListener('keydown', onGlobalKey));
onUnmounted(() => window.removeEventListener('keydown', onGlobalKey));
</script>

<template>
  <nav class="sidebar" aria-label="Навигация" data-testid="app-sidebar">
    <label class="search">
      <Search :size="16" />
      <input ref="searchInput" v-model="query" placeholder="Найти задачу" aria-label="Найти задачу" data-testid="sidebar-search" @keydown="onSearchKey" />
      <kbd>Ctrl K</kbd>
    </label>

    <div class="primary">
      <button class="item" :data-active="view.kind === 'tasks'" data-testid="open-tasks" @click="view = { kind: 'tasks' }">
        <ListTodo :size="16" />
        <span>Задачи</span>
      </button>
      <button class="item" :data-active="view.kind === 'agents'" data-testid="open-agents" @click="view = { kind: 'agents' }">
        <Bot :size="16" />
        <span>Мои агенты</span>
      </button>
    </div>

    <div class="sections">
      <section v-for="w in workspaces.length ? workspaces : workspace ? [workspace] : []" :key="w.id">
        <button
          class="group-label"
          :aria-expanded="w.id === state.workspaceId"
          :data-testid="`workspace-section-${w.id}`"
          :title="w.id === state.workspaceId ? w.name : `Перейти в ${w.name}`"
          @click="w.id !== state.workspaceId && emit('switch', w.id)"
        >
          <span class="badge">{{ w.name.slice(0, 1).toUpperCase() }}</span>
          <span>{{ w.name }}</span>
        </button>
        <template v-if="w.id === state.workspaceId">
          <button
            v-for="task in channels"
            :key="task.id"
            class="item channel"
            :class="{ unread: unread.has(task.id) }"
            :data-active="task.id === selectedId"
            :aria-current="task.id === selectedId ? 'page' : undefined"
            :title="`${task.externalRef} ${task.title}`"
            :data-testid="`channel-${task.externalRef}`"
            @click="openTask(task.id)"
          >
            <span class="key">{{ task.externalRef }}</span>
            <span class="name">{{ task.title }}</span>
            <span v-if="working(task)" class="working">агент</span>
            <span v-if="unread.has(task.id)" class="unread-dot" :data-testid="`channel-unread-${task.externalRef}`"><span class="sr">новое</span></span>
          </button>
          <p v-if="!channels.length" class="empty">{{ query ? 'Ничего не найдено' : 'На вас нет открытых задач' }}</p>
        </template>
      </section>
    </div>

    <footer>
      <span class="me">
        <Avatar v-if="state.me" :id="state.me.id" :name="state.me.name" />
        <span class="presence vr-dot" :data-online="state.connected" :title="state.connected ? 'в сети' : 'нет связи'" />
      </span>
      <span class="who">
        <strong>{{ state.me?.name }}</strong>
        <span>{{ workspace?.name }}</span>
      </span>
      <button class="icon" :title="dark ? 'Светлая тема' : 'Тёмная тема'" :aria-label="dark ? 'Светлая тема' : 'Тёмная тема'" data-testid="toggle-theme" @click="emit('theme')">
        <Sun v-if="dark" :size="16" />
        <Moon v-else :size="16" />
      </button>
      <button class="icon" title="Подключить runner" aria-label="Подключить runner" data-testid="connect-runner" @click="emit('runner')"><Plug :size="16" /></button>
      <button class="icon" title="Выйти" aria-label="Выйти" data-testid="logout" @click="client.logout()"><LogOut :size="16" /></button>
    </footer>
  </nav>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: var(--vr-space-2) var(--vr-space-2) 0;
  color: var(--vr-color-text);
}

.search {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  margin-bottom: var(--vr-space-2);
  border-radius: var(--vr-radius-md);
  background: var(--vr-color-nav-hover);
  color: var(--vr-color-nav-muted);
}

.search input {
  flex: 1;
  min-width: 0;
  font: inherit;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  outline: none;
}

.search input::placeholder {
  color: var(--vr-color-nav-muted);
}

kbd {
  font: inherit;
  font-size: var(--vr-font-size-2xs);
}

.item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  font: inherit;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.item:hover {
  background: var(--vr-color-nav-hover);
}

.item[data-active='true'] {
  font-weight: 600;
  background: var(--vr-color-nav-active);
}

.sections {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  margin: var(--vr-space-3) calc(-1 * var(--vr-space-2)) 0;
  padding: 0 var(--vr-space-2);
}

section + section {
  margin-top: var(--vr-space-3);
}

.group-label {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-nav-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.group-label[aria-expanded='false']:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  font-size: 10px;
  font-weight: 700;
  border-radius: 4px;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-chrome);
}

.channel .key {
  flex: none;
  font-size: var(--vr-font-size-xs);
  font-variant-numeric: tabular-nums;
  color: var(--vr-color-nav-muted);
}

.channel .name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.channel.unread .name {
  font-weight: 600;
}

.working {
  flex: none;
  padding: 2px 6px;
  font-size: var(--vr-font-size-2xs);
  font-weight: 500;
  line-height: 1;
  border-radius: var(--vr-radius-full);
  color: var(--vr-color-accent);
  background: var(--vr-color-chip-bg);
}

.unread-dot {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--vr-color-accent);
}

.sr {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}

.empty {
  margin: 0;
  padding: var(--vr-space-1) var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-nav-muted);
}

footer {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-1);
}

.me {
  position: relative;
  display: inline-flex;
}

.presence {
  position: absolute;
  right: -1px;
  bottom: -1px;
  width: 10px;
  height: 10px;
  box-shadow: 0 0 0 2px var(--vr-color-canvas-bottom);
}

.who {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  line-height: 16px;
}

.who > * {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.who span {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-nav-muted);
}

.icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  color: var(--vr-color-chrome);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.icon:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}
</style>
