<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch, type Component } from 'vue';
import { BookOpen, LayoutPanelLeft, Moon, PanelLeftClose, PanelLeftOpen, Search, Settings, Sun, Ticket } from 'lucide-vue-next';
import { useClientState } from '@core/vue-bindings';
import Avatar from './Avatar.vue';
import { useConnectors } from './connectors/api';
import { useForgeInbox } from './forge/inbox';
import { shortcutLabel } from './commands/keymap';
import { useI18n } from './i18n';
import { navigate, view } from './nav';
import { sidebarCollapsed, sidebarSearchRequests, toggleSidebar } from './sidebarState';
import ProjectTree from './workbench/ProjectTree.vue';
import { treeQuery, useWorkTree } from './workbench/useWorkTree';

defineProps<{ dark: boolean }>();
const emit = defineEmits<{ switch: [id: string]; theme: [] }>();

const { t, locale, setLocale } = useI18n();
const state = useClientState();
const searchInput = ref<HTMLInputElement | null>(null);
const sidebarEl = ref<HTMLElement | null>(null);

const MIN_WIDTH = 180;
const MAX_WIDTH = 400;
const DEFAULT_WIDTH = 300;
const RAIL_WIDTH = 56;
const KEY_STEP = 16;
const WIDTH_KEY = 'veyra.sidebar.tree.width';
const EXPLORER_WIDTH_KEY = 'veyra.agents.a.width';

function clampWidth(value: number) {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(value)));
}

function readWidth() {
  const stored = Number(localStorage.getItem(WIDTH_KEY) ?? localStorage.getItem(EXPLORER_WIDTH_KEY));
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
const { startPolling } = useForgeInbox();
const { tabs: workTabs } = useWorkTree();
const reviewCount = computed(() => workTabs.value.find((tab) => tab.id === 'reviews')?.count ?? 0);

const hasForge = computed(() => connectors.value.some((c) => c.kind === 'github' || c.kind === 'gitlab'));
const hasJira = computed(() => connectors.value.some((c) => c.kind === 'jira'));
const hasConfluence = computed(() => connectors.value.some((c) => c.kind === 'confluence'));

type NavItem = { kind: 'agents' | 'jira' | 'confluence'; label: string; icon: Component; testId: string; count?: number; countTitle?: string };

const navItems = computed<NavItem[]>(() => {
  const items: NavItem[] = [
    { kind: 'agents', label: t('shell.nav.agents'), icon: LayoutPanelLeft, testId: 'open-agents', count: hasForge.value ? reviewCount.value : undefined, countTitle: t('shell.nav.reviewsPending') },
  ];
  if (hasJira.value) items.push({ kind: 'jira', label: t('shell.nav.jira'), icon: Ticket, testId: 'nav-jira' });
  if (hasConfluence.value) items.push({ kind: 'confluence', label: t('shell.nav.confluence'), icon: BookOpen, testId: 'nav-confluence' });
  return items;
});

let stopPolling: (() => void) | undefined;
watch(hasForge, (on) => {
  stopPolling?.();
  stopPolling = on ? startPolling() : undefined;
}, { immediate: true });

const workspace = computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId));

function onSearchKey(event: KeyboardEvent) {
  if (event.key === 'Escape') treeQuery.value = '';
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
        <input ref="searchInput" v-model="treeQuery" :placeholder="t('workbench.filter')" :aria-label="t('workbench.filter')" data-testid="sidebar-search" @keydown="onSearchKey" />
        <kbd v-if="shortcutLabel('sidebar.search')">{{ shortcutLabel('sidebar.search') }}</kbd>
      </label>
      <button v-else class="item rail-item" :title="t('workbench.filter')" :aria-label="t('workbench.filter')" data-testid="sidebar-search-open" @click="openSearch">
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
      <div v-if="state.workspaces.length > 1" class="relay">
        <button
          v-for="w in state.workspaces"
          :key="w.id"
          class="group-label"
          :aria-pressed="w.id === state.workspaceId"
          :data-testid="`workspace-section-${w.id}`"
          :title="w.id === state.workspaceId ? w.name : t('shell.switchWorkspace', { name: w.name })"
          @click="w.id !== state.workspaceId && emit('switch', w.id)"
        >
          <span class="badge">{{ w.name.slice(0, 1).toUpperCase() }}</span>
          <span>{{ w.name }}</span>
        </button>
      </div>
      <ProjectTree />
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
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  margin: var(--vr-space-2) calc(-1 * var(--vr-space-1)) 0;
  overflow: hidden;
}

.relay {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  gap: var(--vr-space-1);
  padding: 0 var(--vr-space-1) var(--vr-space-1);
}

.relay .group-label {
  width: auto;
}

.relay .group-label[aria-pressed='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
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

.group-label[aria-pressed='false']:hover {
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

.icon.lang {
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
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
