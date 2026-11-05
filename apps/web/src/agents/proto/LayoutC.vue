<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef } from 'vue';
import type { DockviewApi } from 'dockview-vue';
import { Folder, Layers, PanelLeft, Search, SquareArrowOutUpRight } from 'lucide-vue-next';
import type { SessionNode } from '../pacer';
import NewSessionForm from '../NewSessionForm.vue';
import ProjectActions from '../manage/ProjectActions.vue';
import WorkspaceActions from '../manage/WorkspaceActions.vue';
import DockArea from './DockArea.vue';
import NavNotices from './NavNotices.vue';
import QuickSwitcher from './QuickSwitcher.vue';
import WorktreeBlock from './WorktreeBlock.vue';
import { NAVIGATOR_PANEL, useAgentsDock } from './useAgentsDock';

const NAVIGATOR_WIDTH = 300;

const state = useAgentsDock({
  init: (api) => addNavigator(api),
  position: (session, api, paths) => {
    const worktree = paths.get(session.key)?.worktree.id;
    const sibling = api.panels.find((panel) => panel.id !== session.key && paths.get(panel.id)?.worktree.id === worktree);
    if (sibling) return { referencePanel: sibling.id, direction: 'within' };
    const lastSession = api.panels.filter((panel) => paths.has(panel.id)).at(-1);
    if (lastSession) return { referencePanel: lastSession.id, direction: 'right' };
    const navigator = api.getPanel(NAVIGATOR_PANEL);
    if (navigator && navigator.group.api.location.type === 'grid') return { referencePanel: NAVIGATOR_PANEL, direction: 'right' };
    return undefined;
  },
});
const { pacer, dock, activeKey, paths } = state;

const switching = shallowRef(false);
const entries = computed(() => [...paths.value.values()]);

function addNavigator(api: DockviewApi) {
  api.addPanel({ id: NAVIGATOR_PANEL, component: 'navigator', title: 'Навигатор', initialWidth: NAVIGATOR_WIDTH });
}

function showNavigator() {
  const api = dock.value;
  if (!api) return;
  const existing = api.getPanel(NAVIGATOR_PANEL);
  if (existing) existing.api.setActive();
  else api.addPanel({ id: NAVIGATOR_PANEL, component: 'navigator', title: 'Навигатор', initialWidth: NAVIGATOR_WIDTH, position: { direction: 'left' } });
}

function floatNavigator() {
  const api = dock.value;
  if (!api) return;
  const panel = api.getPanel(NAVIGATOR_PANEL);
  if (!panel) {
    showNavigator();
    return;
  }
  if (panel.group.api.location.type === 'floating') return;
  api.addFloatingGroup(panel, { x: 24, y: 48, width: NAVIGATOR_WIDTH + 40, height: 520 });
}

function pick(session: SessionNode) {
  switching.value = false;
  state.open(session);
}

function onWindowKey(event: KeyboardEvent) {
  if (!state.visible.value || !event.ctrlKey || event.shiftKey || event.altKey || event.metaKey || event.code !== 'KeyP') return;
  event.preventDefault();
  event.stopPropagation();
  switching.value = !switching.value;
}

onMounted(() => window.addEventListener('keydown', onWindowKey, true));
onBeforeUnmount(() => window.removeEventListener('keydown', onWindowKey, true));
</script>

<template>
  <div class="layout">
    <div class="toolbar">
      <button class="tool" title="Показать навигатор" data-testid="navigator-show" @click="showNavigator"><PanelLeft :size="14" />Навигатор</button>
      <button class="tool" title="Вынести навигатор в плавающее окно" data-testid="navigator-float" @click="floatNavigator"><SquareArrowOutUpRight :size="14" />Плавающий</button>
      <button class="tool" title="Быстрый переход (Ctrl+P)" data-testid="quick-switch-open" @click="switching = true"><Search :size="14" />Перейти<kbd>Ctrl+P</kbd></button>
      <span class="spacer" />
      <span class="tip">Shift + перетаскивание вкладки: плавающая группа</span>
      <button class="tool" title="Использование памяти" data-testid="open-metrics" @click="state.openMetrics()">Память</button>
    </div>
    <DockArea :active-key="activeKey" :paths="paths" group-labels @ready="state.onReady">
      <template #navigator>
        <aside class="navigator" aria-label="Сессии pacer" data-testid="navigator">
          <NavNotices />
          <section v-for="workspace in pacer?.tree.value ?? []" :key="workspace.id" class="workspace">
            <div class="ws-head">
              <Layers :size="14" class="icon" />
              <h2 class="ws-name">{{ workspace.name }}</h2>
              <WorkspaceActions :workspace-id="workspace.id" />
            </div>
            <div v-for="project in workspace.projects" :key="project.id" class="project">
              <div class="project-head">
                <Folder :size="14" class="icon" />
                <h3 class="project-name" :title="project.name">{{ project.name }}</h3>
                <ProjectActions :project-id="project.id" @resumed="state.openWhenListed({ Agent: $event })" />
              </div>
              <WorktreeBlock
                v-for="worktree in project.worktrees"
                :key="worktree.id"
                :worktree="worktree"
                :active-key="activeKey"
                @open="state.open"
                @create="state.creatingIn.value = worktree.id"
              />
            </div>
            <ProjectActions :workspace-id="workspace.id" />
          </section>
          <p v-if="pacer?.connection.value.state === 'connected' && !pacer.sessions.value.size" class="empty">Нет сессий</p>
          <WorkspaceActions class="new-workspace" />
        </aside>
      </template>
    </DockArea>
    <QuickSwitcher v-if="switching" :entries="entries" @select="pick" @close="switching = false" />
    <NewSessionForm v-if="state.creatingWorktree.value" :key="state.creatingWorktree.value.id" :worktree="state.creatingWorktree.value" @close="state.creatingIn.value = null" @created="state.onCreated" />
  </div>
</template>

<style scoped>
.layout {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.toolbar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-1);
  height: 30px;
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  border-bottom: 1px solid var(--vr-color-border);
}

.spacer {
  flex: 1;
}

.tip {
  color: var(--vr-color-text-muted);
}

.tool {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 22px;
  padding: 0 var(--vr-space-2);
  font: inherit;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.tool:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

kbd {
  margin-left: var(--vr-space-1);
  padding: 0 var(--vr-space-1);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-sm);
}

.navigator {
  height: 100%;
  padding: var(--vr-space-2);
  overflow-y: auto;
  background: var(--vr-color-surface);
}

.workspace + .workspace {
  margin-top: var(--vr-space-4);
}

.ws-head,
.project-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
}

.icon {
  flex: none;
  color: var(--vr-color-text-muted);
}

.ws-name,
.project-name {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ws-name {
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--vr-color-text-muted);
  line-height: var(--vr-size-row);
}

.project {
  margin-top: var(--vr-space-2);
}

.project-name {
  font-size: var(--vr-font-size-base);
  font-weight: 600;
  line-height: var(--vr-size-row);
}

.new-workspace {
  margin-top: var(--vr-space-3);
}

.empty {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
