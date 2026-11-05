<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch, type Component, type Directive } from 'vue';
import { BookOpen, GitPullRequest, Hash, ListTodo, Moon, PanelLeftClose, PanelLeftOpen, Plus, Search, Settings, SquareTerminal, Sun, Ticket } from 'lucide-vue-next';
import { useClient, useClientState } from '@core/vue-bindings';
import Avatar from './Avatar.vue';
import { useConnectors } from './connectors/api';
import { useForgeInbox } from './forge/inbox';
import { shortcutLabel } from './commands/keymap';
import { useI18n } from './i18n';
import { navigate, openTask, openTopic, view } from './nav';
import { priorityRank } from './priority';
import { sidebarCollapsed, sidebarSearchRequests, toggleSidebar } from './sidebarState';

defineProps<{ dark: boolean }>();
const emit = defineEmits<{ switch: [id: string]; theme: [] }>();

const { t, locale, setLocale } = useI18n();
const client = useClient();
const state = useClientState();
const query = ref('');
const creating = ref(false);
const topicName = ref('');
const topicError = ref('');
const vFocus: Directive<HTMLElement> = { mounted: (el) => el.focus() };
const searchInput = ref<HTMLInputElement | null>(null);
const sidebarEl = ref<HTMLElement | null>(null);

const MIN_WIDTH = 180;
const MAX_WIDTH = 400;
const DEFAULT_WIDTH = 300;
const RAIL_WIDTH = 56;
const KEY_STEP = 16;
const WIDTH_KEY = 'veyra.sidebar.width';

function clampWidth(value: number) {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(value)));
}

function readWidth() {
  const stored = Number(localStorage.getItem(WIDTH_KEY));
  return Number.isFinite(stored) && stored > 0 ? clampWidth(stored) : DEFAULT_WIDTH;
}

const width = ref(readWidth());
const collapsed = sidebarCollapsed;
const dragging = ref(false);
const currentWidth = computed(() => (collapsed.value ? RAIL_WIDTH : width.value));
const collapseLabel = computed(() => (collapsed.value ? t('shell.expand') : t('shell.collapse')));
const nextLocale = computed(() => (locale.value === 'en' ? 'ru' : 'en'));

watch(width, (value) => localStorage.setItem(WIDTH_KEY, String(value)));

async function openSearch() {
  collapsed.value = false;
  await nextTick();
  searchInput.value?.focus();
}

watch(sidebarSearchRequests, () => void openSearch());

function shortcutTitle(label: string, actionId: string) {
  const shortcut = shortcutLabel(actionId);
  return shortcut ? `${label} (${shortcut})` : label;
}

function startResize(event: PointerEvent) {
  if (event.button !== 0 || !(event.currentTarget instanceof HTMLElement)) return;
  event.preventDefault();
  event.currentTarget.setPointerCapture(event.pointerId);
  dragging.value = true;
}

function moveResize(event: PointerEvent) {
  if (!dragging.value || !sidebarEl.value) return;
  const edge = event.clientX - sidebarEl.value.getBoundingClientRect().left;
  collapsed.value = edge < MIN_WIDTH;
  if (!collapsed.value) width.value = clampWidth(edge);
}

function endResize() {
  dragging.value = false;
}

function onHandleKey(event: KeyboardEvent) {
  const step = event.key === 'ArrowLeft' ? -KEY_STEP : event.key === 'ArrowRight' ? KEY_STEP : 0;
  if (!step) return;
  event.preventDefault();
  if (collapsed.value) {
    collapsed.value = step < 0;
    return;
  }
  const next = width.value + step;
  if (next < MIN_WIDTH) collapsed.value = true;
  else width.value = clampWidth(next);
}

const { connectors, list: listConnectors } = useConnectors();
const { reviewCount, startPolling } = useForgeInbox();

const hasForge = computed(() => connectors.value.some((c) => c.kind === 'github' || c.kind === 'gitlab'));
const hasJira = computed(() => connectors.value.some((c) => c.kind === 'jira'));
const hasConfluence = computed(() => connectors.value.some((c) => c.kind === 'confluence'));

type NavItem = { kind: 'tasks' | 'agents' | 'reviews' | 'jira' | 'confluence'; label: string; icon: Component; testId: string; count?: number; countTitle?: string };

const navItems = computed<NavItem[]>(() => {
  const items: NavItem[] = [
    { kind: 'tasks', label: t('shell.nav.tasks'), icon: ListTodo, testId: 'open-tasks' },
    { kind: 'agents', label: t('shell.nav.agents'), icon: SquareTerminal, testId: 'open-agents' },
  ];
  if (hasForge.value) items.push({ kind: 'reviews', label: t('shell.nav.reviews'), icon: GitPullRequest, testId: 'nav-reviews', count: reviewCount.value, countTitle: t('shell.nav.reviewsPending') });
  if (hasJira.value) items.push({ kind: 'jira', label: t('shell.nav.jira'), icon: Ticket, testId: 'nav-jira' });
  if (hasConfluence.value) items.push({ kind: 'confluence', label: t('shell.nav.confluence'), icon: BookOpen, testId: 'nav-confluence' });
  return items;
});

let stopPolling: (() => void) | undefined;
watch(hasForge, (on) => {
  stopPolling?.();
  stopPolling = on ? startPolling() : undefined;
}, { immediate: true });

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

function startTopic() {
  creating.value = true;
  topicError.value = '';
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

onMounted(() => {
  void listConnectors();
});
onUnmounted(() => {
  stopPolling?.();
});
</script>

<template>
  <nav
    ref="sidebarEl"
    class="sidebar"
    :class="{ collapsed, dragging }"
    :style="{ width: `${currentWidth}px` }"
    :aria-label="t('shell.nav.label')"
    data-testid="sidebar"
    :data-collapsed="collapsed"
  >
    <div class="top">
      <label v-if="!collapsed" class="search">
        <Search :size="16" />
        <input ref="searchInput" v-model="query" :placeholder="t('shell.search')" :aria-label="t('shell.search')" data-testid="sidebar-search" @keydown="onSearchKey" />
        <kbd v-if="shortcutLabel('sidebar.search')">{{ shortcutLabel('sidebar.search') }}</kbd>
      </label>
      <button v-else class="item rail-item" :title="t('shell.search')" :aria-label="t('shell.search')" data-testid="sidebar-search-open" @click="openSearch">
        <Search :size="16" />
      </button>
      <button
        class="icon toggle"
        :title="shortcutTitle(collapseLabel, 'sidebar.toggle')"
        :aria-label="collapseLabel"
        :aria-expanded="!collapsed"
        data-testid="sidebar-collapse"
        @click="toggleSidebar"
      >
        <PanelLeftOpen v-if="collapsed" :size="16" />
        <PanelLeftClose v-else :size="16" />
      </button>
    </div>

    <div class="primary">
      <button
        v-for="item in navItems"
        :key="item.kind"
        class="item vt-nav"
        :class="{ 'rail-item': collapsed }"
        :data-active="view.kind === item.kind"
        :title="collapsed ? item.label : undefined"
        :aria-label="collapsed ? item.label : undefined"
        :data-testid="item.testId"
        @click="navigate({ kind: item.kind })"
      >
        <component :is="item.icon" :size="16" />
        <span v-if="!collapsed">{{ item.label }}</span>
        <span v-if="item.count" class="count" :title="item.countTitle" :data-testid="item.testId + '-count'">{{ item.count }}</span>
      </button>
    </div>

    <div v-if="!collapsed" class="sections">
      <section v-for="w in state.workspaces" :key="w.id">
        <button
          class="group-label"
          :aria-expanded="w.id === state.workspaceId"
          :data-testid="`workspace-section-${w.id}`"
          :title="w.id === state.workspaceId ? w.name : t('shell.switchWorkspace', { name: w.name })"
          @click="w.id !== state.workspaceId && emit('switch', w.id)"
        >
          <span class="badge">{{ w.name.slice(0, 1).toUpperCase() }}</span>
          <span>{{ w.name }}</span>
        </button>
        <template v-if="w.id === state.workspaceId">
          <div class="subhead">
            <span>{{ t('shell.topics') }}</span>
            <button class="icon add" :title="t('shell.newTopic')" :aria-label="t('shell.newTopic')" data-testid="topic-add" @click="startTopic"><Plus :size="14" /></button>
          </div>
          <form v-if="creating" class="new-topic" @submit.prevent="createTopic">
            <Hash :size="14" />
            <input v-model="topicName" v-focus :placeholder="t('shell.topicPlaceholder')" :aria-label="t('shell.topicName')" data-testid="topic-name" @keydown.esc="creating = false" />
          </form>
          <p v-if="topicError" class="empty" role="alert">{{ topicError }}</p>
          <div v-auto-animate>
            <button
              v-for="topic in state.topics"
              :key="topic.id"
              class="item vt-nav channel"
              :class="{ unread: state.unread.has(topic.id) }"
              :data-active="topic.id === selectedTopicId"
              :aria-current="topic.id === selectedTopicId ? 'page' : undefined"
              :data-testid="`topic-${topic.name}`"
              @click="openTopic(topic.id)"
            >
              <Hash :size="14" class="key" />
              <span class="name">{{ topic.name }}</span>
              <span v-if="state.unread.has(topic.id)" class="unread-dot" :data-testid="`topic-unread-${topic.name}`"><span class="sr">{{ t('shell.unread') }}</span></span>
            </button>
          </div>
          <div class="subhead"><span>{{ t('shell.tasks') }}</span></div>
          <div v-auto-animate>
            <button
              v-for="task in channels"
              :key="task.id"
              class="item vt-nav channel"
              :class="{ unread: state.unread.has(task.id) }"
              :data-active="task.id === selectedId"
              :aria-current="task.id === selectedId ? 'page' : undefined"
              :title="`${task.externalRef} ${task.title}`"
              :data-testid="`channel-${task.externalRef}`"
              @click="openTask(task.id)"
            >
              <span class="key">{{ task.externalRef }}</span>
              <span class="name">{{ task.title }}</span>
              <span v-if="state.unread.has(task.id)" class="unread-dot" :data-testid="`channel-unread-${task.externalRef}`"><span class="sr">{{ t('shell.unread') }}</span></span>
            </button>
          </div>
          <p v-if="!channels.length" class="empty">{{ query ? t('shell.nothingFound') : t('shell.noOpenTasks') }}</p>
        </template>
      </section>
    </div>

    <footer>
      <span class="me">
        <Avatar :id="state.me.id" :name="state.me.name" />
        <span class="presence vr-dot" :data-online="state.connected" :title="state.connected ? t('shell.online') : t('shell.offline')" />
      </span>
      <span v-if="!collapsed" class="who">
        <strong>{{ state.me.name }}</strong>
        <span>{{ workspace?.name }}</span>
      </span>
      <button
        class="icon"
        :data-active="view.kind === 'settings'"
        :title="shortcutTitle(t('shell.settings'), 'nav.settings')"
        :aria-label="t('shell.settings')"
        data-testid="open-settings"
        @click="navigate({ kind: 'settings' })"
      >
        <Settings :size="16" />
      </button>
      <button v-if="!collapsed" class="icon lang" :title="t(`common.language.${nextLocale}`)" :aria-label="t('common.language.label')" data-testid="toggle-language" @click="setLocale(nextLocale)">
        {{ locale.toUpperCase() }}
      </button>
      <button class="icon" :title="dark ? t('shell.themeLight') : t('shell.themeDark')" :aria-label="dark ? t('shell.themeLight') : t('shell.themeDark')" data-testid="toggle-theme" @click="emit('theme')">
        <Sun v-if="dark" :size="16" />
        <Moon v-else :size="16" />
      </button>
    </footer>

    <div
      class="resize-handle"
      role="separator"
      aria-orientation="vertical"
      :aria-label="t('shell.resize')"
      tabindex="0"
      :aria-valuenow="currentWidth"
      :aria-valuemin="RAIL_WIDTH"
      :aria-valuemax="MAX_WIDTH"
      data-testid="sidebar-resize-handle"
      @pointerdown="startResize"
      @pointermove="moveResize"
      @pointerup="endResize"
      @pointercancel="endResize"
      @keydown="onHandleKey"
    />
  </nav>
</template>

<style scoped>
.sidebar {
  position: relative;
  display: flex;
  flex: none;
  flex-direction: column;
  box-sizing: border-box;
  min-height: 0;
  min-width: 0;
  padding: var(--vr-space-2) var(--vr-space-2) 0;
  overflow: hidden;
  color: var(--vr-color-text);
  transition: width 0.18s ease;
}

.sidebar.dragging {
  transition: none;
  user-select: none;
}

.top {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  margin-bottom: var(--vr-space-2);
}

.collapsed .top {
  flex-direction: column-reverse;
  align-items: stretch;
}

.collapsed .top .toggle {
  align-self: center;
}

.item.rail-item {
  justify-content: center;
  padding: 0;
}

.toggle {
  flex: none;
}

.collapsed .primary {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
}

.collapsed footer {
  flex-direction: column;
  margin-top: auto;
  padding: var(--vr-space-3) 0;
}

.resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 2;
  width: 6px;
  cursor: col-resize;
  touch-action: none;
}

.resize-handle::after {
  content: '';
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 2px;
  background: transparent;
  transition: background 0.12s ease;
}

.resize-handle:hover::after,
.resize-handle:focus-visible::after,
.dragging .resize-handle::after {
  background: var(--vr-color-accent);
}

.resize-handle:focus-visible {
  outline: none;
}

.search {
  display: flex;
  flex: 1;
  min-width: 0;
  align-items: center;
  gap: var(--vr-space-2);
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
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

.item {
  position: relative;
}

.count {
  min-width: 16px;
  padding: 0 var(--vr-space-1);
  margin-left: auto;
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
  line-height: 16px;
  text-align: center;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
  border-radius: var(--vr-radius-full);
}

.rail-item .count {
  position: absolute;
  top: 2px;
  right: 6px;
  margin-left: 0;
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
  overflow-x: hidden;
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

.icon.lang {
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
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

.icon:hover,
.icon[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}
</style>
