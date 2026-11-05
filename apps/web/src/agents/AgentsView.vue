<script setup lang="ts">
import 'dockview-vue/dist/styles/dockview.css';
import { computed, onActivated, onBeforeUnmount, onDeactivated, shallowRef, useTemplateRef, watch } from 'vue';
import { DockviewVue, themeLight, type DockviewApi, type DockviewReadyEvent } from 'dockview-vue';
import { ChevronRight, ChevronsLeft, ChevronsRight, Plus, X } from 'lucide-vue-next';
import { isSessionRef, markSeen, sessionKey, usePacer, type SessionNode, type SessionRef } from './pacer';
import ArchivedRow from './ArchivedRow.vue';
import NewSessionForm from './NewSessionForm.vue';
import SessionPanel from './SessionPanel.vue';
import SessionRow from './SessionRow.vue';
import StatusDot from './StatusDot.vue';
import MetricsPanel from './manage/MetricsPanel.vue';
import ProjectActions from './manage/ProjectActions.vue';
import WorkspaceActions from './manage/WorkspaceActions.vue';

const METRICS_PANEL = 'metrics';
const TREE_WIDTH_KEY = 'veyra.pacer.treeWidth';
const TREE_COLLAPSED_KEY = 'veyra.pacer.treeCollapsed';
const TREE_MIN_WIDTH = 180;
const TREE_DEFAULT_WIDTH = 280;
const TREE_KEY_STEP = 16;

const pacer = usePacer();
const dock = shallowRef<DockviewApi | null>(null);
const activeKey = shallowRef<string | null>(null);
const visible = shallowRef(true);
let activeChange: { dispose(): void } | null = null;

const view = useTemplateRef<HTMLDivElement>('view');
const storedWidth = Number(localStorage.getItem(TREE_WIDTH_KEY));
const treeWidth = shallowRef(storedWidth >= TREE_MIN_WIDTH ? storedWidth : TREE_DEFAULT_WIDTH);
const treeCollapsed = shallowRef(localStorage.getItem(TREE_COLLAPSED_KEY) === '1');
const columns = computed(() => (treeCollapsed.value ? 'auto minmax(0, 1fr)' : `min(${treeWidth.value}px, 50%) minmax(0, 1fr)`));

function setTreeWidth(width: number) {
  const max = Math.max(TREE_MIN_WIDTH, (view.value?.clientWidth ?? Number.POSITIVE_INFINITY) / 2);
  treeWidth.value = Math.round(Math.min(max, Math.max(TREE_MIN_WIDTH, width)));
}

function saveTreeWidth() {
  localStorage.setItem(TREE_WIDTH_KEY, String(treeWidth.value));
}

function startResize(event: PointerEvent) {
  const handle = event.currentTarget;
  if (!(handle instanceof HTMLElement)) return;
  event.preventDefault();
  const startX = event.clientX;
  const startWidth = Math.min(treeWidth.value, handle.parentElement?.clientWidth ?? treeWidth.value);
  handle.setPointerCapture(event.pointerId);
  const move = (next: PointerEvent) => setTreeWidth(startWidth + next.clientX - startX);
  const end = () => {
    handle.removeEventListener('pointermove', move);
    handle.removeEventListener('pointerup', end);
    handle.removeEventListener('pointercancel', end);
    saveTreeWidth();
  };
  handle.addEventListener('pointermove', move);
  handle.addEventListener('pointerup', end);
  handle.addEventListener('pointercancel', end);
}

function resizeByKey(event: KeyboardEvent) {
  const step = event.key === 'ArrowLeft' ? -TREE_KEY_STEP : event.key === 'ArrowRight' ? TREE_KEY_STEP : 0;
  if (!step) return;
  event.preventDefault();
  setTreeWidth(treeWidth.value + step);
  saveTreeWidth();
}

function setTreeCollapsed(collapsed: boolean) {
  treeCollapsed.value = collapsed;
  localStorage.setItem(TREE_COLLAPSED_KEY, collapsed ? '1' : '0');
}

const creatingIn = shallowRef<string | null>(null);
const creatingWorktree = computed(() => {
  const id = creatingIn.value;
  if (!id) return null;
  for (const workspace of pacer?.tree.value ?? []) for (const project of workspace.projects) for (const worktree of project.worktrees) if (worktree.id === id) return worktree;
  return null;
});
const openingKey = shallowRef<string | null>(null);
const openArchives = shallowRef<ReadonlySet<string>>(new Set());

function toggleArchive(worktree: string) {
  const next = new Set(openArchives.value);
  if (!next.delete(worktree)) next.add(worktree);
  openArchives.value = next;
}

function onCreated(session: SessionRef) {
  creatingIn.value = null;
  openWhenListed(session);
}

function openWhenListed(session: SessionRef) {
  const key = sessionKey(session);
  const node = pacer?.sessions.value.get(key);
  if (node) open(node);
  else openingKey.value = key;
}

const banner = computed(() => {
  const connection = pacer?.connection.value;
  if (!connection) return null;
  switch (connection.state) {
    case 'connected':
      return null;
    case 'connecting':
      return 'подключаюсь к pacer…';
    case 'offline':
      return 'демон pacer не запущен';
    case 'incompatible':
      return connection.message ?? 'несовместимая версия pacer';
    default: {
      const unknown: never = connection.state;
      return unknown;
    }
  }
});

const activeUnseenAgent = computed(() => {
  const session = activeKey.value ? pacer?.sessions.value.get(activeKey.value) : undefined;
  return visible.value && session?.unseen && 'Agent' in session.ref ? session.ref.Agent : null;
});

watch(activeUnseenAgent, (agent) => {
  if (agent) void markSeen(agent);
});

watch(
  () => pacer?.sessions.value,
  (sessions) => {
    const api = dock.value;
    if (!api || !sessions || pacer?.connection.value.state !== 'connected') return;
    for (const panel of api.panels) if (panel.id !== METRICS_PANEL && !sessions.has(panel.id)) panel.api.close();
    const opening = openingKey.value ? sessions.get(openingKey.value) : undefined;
    if (!opening) return;
    openingKey.value = null;
    open(opening);
  },
);

function onReady({ api }: DockviewReadyEvent) {
  dock.value = api;
  activeChange = api.onDidActivePanelChange(({ panel }) => {
    activeKey.value = panel?.id ?? null;
  });
}

function openPanel(panel: Parameters<DockviewApi['addPanel']>[0]) {
  const api = dock.value;
  if (!api) return;
  const existing = api.getPanel(panel.id);
  if (existing) {
    existing.api.setActive();
    return;
  }
  api.addPanel(panel);
}

function open(session: SessionNode) {
  openPanel({ id: session.key, component: 'session', title: session.name, params: { session: session.ref }, renderer: 'always' });
}

function openMetrics() {
  openPanel({ id: METRICS_PANEL, component: 'metrics', title: 'Память' });
}

onActivated(() => {
  visible.value = true;
});
onDeactivated(() => {
  visible.value = false;
});
onBeforeUnmount(() => activeChange?.dispose());
</script>

<template>
  <div v-if="!pacer" class="unavailable" data-testid="pacer-banner" data-state="unavailable">Сессии pacer работают только в десктопном приложении Veyra</div>
  <div v-else ref="view" class="agents" :style="{ gridTemplateColumns: columns }">
    <div v-if="treeCollapsed" class="rail">
      <button class="icon" title="Показать сессии" aria-label="Показать сессии" data-testid="tree-expand" @click="setTreeCollapsed(false)"><ChevronsRight :size="14" /></button>
    </div>
    <div v-else class="tree-pane">
      <div class="tree-head">
        <span>Сессии</span>
        <button class="metrics-open" title="Использование памяти" data-testid="open-metrics" @click="openMetrics">Память</button>
        <button class="icon" title="Скрыть сессии" aria-label="Скрыть сессии" data-testid="tree-collapse" @click="setTreeCollapsed(true)"><ChevronsLeft :size="14" /></button>
      </div>
      <aside class="tree" aria-label="Сессии pacer">
        <p v-if="banner" class="banner" role="status" data-testid="pacer-banner" :data-state="pacer.connection.value.state">{{ banner }}</p>
        <p v-if="pacer.error.value" class="banner" role="alert" data-testid="pacer-error">{{ pacer.error.value }}</p>
        <section v-for="workspace in pacer.tree.value" :key="workspace.id" class="workspace">
          <div class="workspace-head">
            <h2 class="workspace-name">{{ workspace.name }}</h2>
            <WorkspaceActions :workspace-id="workspace.id" />
          </div>
          <div v-for="project in workspace.projects" :key="project.id" class="project">
            <div class="project-head">
              <div class="project-name" :title="project.name">{{ project.name }}</div>
              <ProjectActions :project-id="project.id" @resumed="openWhenListed({ Agent: $event })" />
            </div>
            <div v-for="worktree in project.worktrees" :key="worktree.id" class="worktree" data-testid="worktree-row" :data-worktree-id="worktree.id">
              <div class="branch">
                <span class="branch-name" :title="worktree.branch">{{ worktree.branch }}</span>
                <button class="icon add" title="Новая сессия" aria-label="Новая сессия" data-testid="worktree-new-session" @click="creatingIn = worktree.id"><Plus :size="14" /></button>
              </div>
              <SessionRow v-for="session in worktree.sessions" :key="session.key" :session="session" :active="session.key === activeKey" @open="open(session)" />
              <template v-if="worktree.archived.length">
                <button class="archive-toggle" :aria-expanded="openArchives.has(worktree.id)" data-testid="archive-toggle" @click="toggleArchive(worktree.id)">
                  <ChevronRight :size="12" class="chevron" />
                  <span>Архив ({{ worktree.archived.length }})</span>
                </button>
                <template v-if="openArchives.has(worktree.id)">
                  <ArchivedRow v-for="session in worktree.archived" :key="session.key" :session="session" />
                </template>
              </template>
            </div>
          </div>
          <ProjectActions :workspace-id="workspace.id" />
        </section>
        <p v-if="pacer.connection.value.state === 'connected' && !pacer.sessions.value.size" class="empty">Нет сессий</p>
        <WorkspaceActions class="new-workspace" />
      </aside>
      <div
        class="resize-handle"
        role="separator"
        aria-orientation="vertical"
        aria-label="Ширина списка сессий"
        tabindex="0"
        :aria-valuenow="treeWidth"
        :aria-valuemin="TREE_MIN_WIDTH"
        data-testid="tree-resize-handle"
        @pointerdown="startResize"
        @keydown="resizeByKey"
      />
    </div>
    <div class="dock">
      <DockviewVue class="dockview" :theme="themeLight" @ready="onReady">
        <template #panel-session="{ params }">
          <SessionPanel v-if="isSessionRef(params.session)" :session="params.session" />
        </template>
        <template #panel-metrics>
          <MetricsPanel />
        </template>
        <template #defaultTab="{ api }">
          <span class="tab" :data-testid="api.id === METRICS_PANEL ? 'metrics-tab' : 'session-tab'" :data-session-id="pacer.sessions.value.get(api.id)?.id" :data-active="api.id === activeKey">
            <StatusDot v-if="api.id !== METRICS_PANEL" :status="pacer.sessions.value.get(api.id)?.status" :unseen="pacer.sessions.value.get(api.id)?.unseen" />
            <span class="name">{{ pacer.sessions.value.get(api.id)?.name ?? api.title }}</span>
            <button class="close" title="Закрыть" aria-label="Закрыть" data-testid="session-tab-close" @pointerdown.stop @click.stop="api.close()"><X :size="12" /></button>
          </span>
        </template>
        <template #watermark>
          <p class="watermark">Выберите сессию слева</p>
        </template>
      </DockviewVue>
    </div>
    <NewSessionForm v-if="creatingWorktree" :key="creatingWorktree.id" :worktree="creatingWorktree" @close="creatingIn = null" @created="onCreated" />
  </div>
</template>

<style scoped>
.unavailable {
  margin: auto;
  color: var(--vr-color-text-muted);
}

.agents {
  display: grid;
  flex: 1;
  min-height: 0;
}

.tree-pane {
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  border-right: 1px solid var(--vr-color-border);
}

.tree-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--vr-space-1) var(--vr-space-2) 0 var(--vr-space-4);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.tree {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-2);
}

.rail {
  padding: var(--vr-space-1);
  border-right: 1px solid var(--vr-color-border);
}

.resize-handle {
  position: absolute;
  top: 0;
  right: -3px;
  bottom: 0;
  z-index: 1;
  width: 6px;
  cursor: col-resize;
  touch-action: none;
}

.resize-handle:hover,
.resize-handle:focus-visible {
  outline: none;
  background: var(--vr-color-nav-active);
}

.icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.icon:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.banner {
  margin: 0 0 var(--vr-space-2);
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  border-radius: var(--vr-radius-md);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}

.banner[role='alert'],
.banner[data-state='offline'],
.banner[data-state='incompatible'] {
  color: var(--vr-color-danger);
}

.workspace + .workspace {
  margin-top: var(--vr-space-3);
}

.workspace-head,
.project-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
}

.workspace-head .workspace-name,
.project-head .project-name {
  flex: 1;
  min-width: 0;
}

.metrics-open {
  margin-left: auto;
  padding: 0 var(--vr-space-2);
  font: inherit;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.metrics-open:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.new-workspace {
  margin-top: var(--vr-space-3);
}

.workspace-name {
  margin: 0;
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  line-height: var(--vr-size-row);
  color: var(--vr-color-text-muted);
}

.project-name,
.branch-name,
.name {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.project-name {
  padding: 0 var(--vr-space-2);
  font-weight: 600;
  line-height: var(--vr-size-row);
}

.branch {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 0 var(--vr-space-1) 0 var(--vr-space-4);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  line-height: var(--vr-size-row);
  color: var(--vr-color-text-muted);
}

.branch-name {
  flex: 1;
  min-width: 0;
}

.branch .add {
  opacity: 0;
}

.branch:hover .add,
.branch .add:focus-visible {
  opacity: 1;
}

.archive-toggle {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  width: 100%;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2) 0 var(--vr-space-5);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  text-align: left;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.archive-toggle:hover {
  background: var(--vr-color-nav-hover);
}

.archive-toggle .chevron {
  transition: transform 0.1s;
}

.archive-toggle[aria-expanded='true'] .chevron {
  transform: rotate(90deg);
}

.empty {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.dock {
  min-width: 0;
  min-height: 0;
}

.dock :deep(.dockview) {
  height: 100%;
}

.dock :deep(.dockview-theme-light) {
  color-scheme: inherit;
  --dv-group-view-background-color: var(--vr-color-surface);
  --dv-tabs-and-actions-container-background-color: var(--vr-color-surface-muted);
  --dv-tabs-and-actions-container-font-size: var(--vr-font-size-xs);
  --dv-activegroup-visiblepanel-tab-background-color: var(--vr-color-surface);
  --dv-activegroup-hiddenpanel-tab-background-color: var(--vr-color-surface-muted);
  --dv-inactivegroup-visiblepanel-tab-background-color: var(--vr-color-surface);
  --dv-inactivegroup-hiddenpanel-tab-background-color: var(--vr-color-surface-muted);
  --dv-activegroup-visiblepanel-tab-color: var(--vr-color-text);
  --dv-activegroup-hiddenpanel-tab-color: var(--vr-color-text-muted);
  --dv-inactivegroup-visiblepanel-tab-color: var(--vr-color-text-muted);
  --dv-inactivegroup-hiddenpanel-tab-color: var(--vr-color-text-muted);
  --dv-tab-divider-color: var(--vr-color-border);
  --dv-separator-border: var(--vr-color-border);
  --dv-paneview-header-border-color: var(--vr-color-border);
  --dv-icon-hover-background-color: var(--vr-color-nav-hover);
  --dv-drag-over-background-color: var(--vr-color-nav-active);
  --dv-scrollbar-background-color: var(--vr-color-nav-active);
  --dv-tabs-container-scrollbar-color: var(--vr-color-nav-active);
}

.tab {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 100%;
  padding: 0 var(--vr-space-1) 0 var(--vr-space-3);
}

.tab .name {
  max-width: 200px;
}

.close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  visibility: hidden;
}

.tab:hover .close,
.tab[data-active='true'] .close {
  visibility: visible;
}

.close:hover {
  background: var(--vr-color-nav-hover);
}

.watermark {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  margin: 0;
  color: var(--vr-color-text-muted);
}
</style>
