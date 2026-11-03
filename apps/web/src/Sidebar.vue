<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue';
import { Hash, ListTodo, Moon, Plus, Search, Sun } from 'lucide-vue-next';
import { useClient, useClientState } from '@core/vue-bindings';
import Avatar from './Avatar.vue';
import { openTask, openTopic, view } from './nav';
import { priorityRank } from './priority';

defineProps<{ dark: boolean }>();
const emit = defineEmits<{ switch: [id: string]; theme: [] }>();

const client = useClient();
const state = useClientState();
const query = ref('');
const creating = ref(false);
const topicName = ref('');
const topicError = ref('');
const topicInput = ref<HTMLInputElement | null>(null);
const searchInput = ref<HTMLInputElement | null>(null);

const selectedId = computed(() => (view.value.kind === 'task' ? view.value.id : null));
const selectedTopicId = computed(() => (view.value.kind === 'topic' ? view.value.id : null));
const workspace = computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId));

const mine = computed(() => state.value.tasks.filter((t) => t.assigneeId === state.value.me.id && t.statusCategory !== 'done'));

const channels = computed(() => {
  const text = query.value.trim().toLowerCase();
  const list = text
    ? state.value.tasks.filter((t) => t.externalRef.toLowerCase().includes(text) || t.title.toLowerCase().includes(text)).slice(0, 50)
    : [...mine.value, ...state.value.tasks.filter((t) => t.id === selectedId.value && !mine.value.includes(t))];
  return [...list].sort((a, b) => priorityRank(a.priority) - priorityRank(b.priority) || a.externalRef.localeCompare(b.externalRef, undefined, { numeric: true }));
});

function onSearchKey(event: KeyboardEvent) {
  if (event.key === 'Escape') query.value = '';
  if (event.key === 'Enter' && channels.value[0]) openTask(channels.value[0].id);
}

async function startTopic() {
  creating.value = true;
  topicError.value = '';
  await nextTick();
  topicInput.value?.focus();
}

async function createTopic() {
  const name = topicName.value.trim();
  if (!name) return;
  topicError.value = '';
  try {
    const topic = await client.createTopic(name);
    topicName.value = '';
    creating.value = false;
    openTopic(topic.id);
  } catch (e) {
    topicError.value = e instanceof Error ? e.message : String(e);
  }
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
    </div>

    <div class="sections">
      <section v-for="w in state.workspaces" :key="w.id">
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
          <div class="subhead">
            <span>Топики</span>
            <button class="icon add" title="Новый топик" aria-label="Новый топик" data-testid="topic-add" @click="startTopic"><Plus :size="14" /></button>
          </div>
          <form v-if="creating" class="new-topic" @submit.prevent="createTopic">
            <Hash :size="14" />
            <input ref="topicInput" v-model="topicName" placeholder="название топика" aria-label="Название топика" data-testid="topic-name" @keydown.esc="creating = false" />
          </form>
          <p v-if="topicError" class="empty" role="alert">{{ topicError }}</p>
          <button
            v-for="topic in state.topics"
            :key="topic.id"
            class="item channel"
            :class="{ unread: state.unread.has(topic.id) }"
            :data-active="topic.id === selectedTopicId"
            :aria-current="topic.id === selectedTopicId ? 'page' : undefined"
            :data-testid="`topic-${topic.name}`"
            @click="openTopic(topic.id)"
          >
            <Hash :size="14" class="key" />
            <span class="name">{{ topic.name }}</span>
            <span v-if="state.unread.has(topic.id)" class="unread-dot" :data-testid="`topic-unread-${topic.name}`"><span class="sr">новое</span></span>
          </button>
          <div class="subhead"><span>Задачи</span></div>
          <button
            v-for="task in channels"
            :key="task.id"
            class="item channel"
            :class="{ unread: state.unread.has(task.id) }"
            :data-active="task.id === selectedId"
            :aria-current="task.id === selectedId ? 'page' : undefined"
            :title="`${task.externalRef} ${task.title}`"
            :data-testid="`channel-${task.externalRef}`"
            @click="openTask(task.id)"
          >
            <span class="key">{{ task.externalRef }}</span>
            <span class="name">{{ task.title }}</span>
            <span v-if="state.unread.has(task.id)" class="unread-dot" :data-testid="`channel-unread-${task.externalRef}`"><span class="sr">новое</span></span>
          </button>
          <p v-if="!channels.length" class="empty">{{ query ? 'Ничего не найдено' : 'На вас нет открытых задач' }}</p>
        </template>
      </section>
    </div>

    <footer>
      <span class="me">
        <Avatar :id="state.me.id" :name="state.me.name" />
        <span class="presence vr-dot" :data-online="state.connected" :title="state.connected ? 'в сети' : 'нет связи'" />
      </span>
      <span class="who">
        <strong>{{ state.me.name }}</strong>
        <span>{{ workspace?.name }}</span>
      </span>
      <button class="icon" :title="dark ? 'Светлая тема' : 'Тёмная тема'" :aria-label="dark ? 'Светлая тема' : 'Тёмная тема'" data-testid="toggle-theme" @click="emit('theme')">
        <Sun v-if="dark" :size="16" />
        <Moon v-else :size="16" />
      </button>
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

.subhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  margin-top: var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-nav-muted);
}

.icon.add {
  width: 20px;
  height: 20px;
}

.new-topic {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  border-radius: var(--vr-radius-md);
  background: var(--vr-color-nav-hover);
  color: var(--vr-color-nav-muted);
}

.new-topic input {
  flex: 1;
  min-width: 0;
  font: inherit;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  outline: none;
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
