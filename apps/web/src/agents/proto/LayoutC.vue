<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, watch } from 'vue';
import type { DockviewApi } from 'dockview-vue';
import { Folder, Layers, PanelLeft, Search, SquareArrowOutUpRight } from 'lucide-vue-next';
import type { SessionNode } from '../pacer';
import { t } from '../../i18n';
import NewSessionForm from '../NewSessionForm.vue';
import ProjectActions from '../manage/ProjectActions.vue';
import WorkspaceActions from '../manage/WorkspaceActions.vue';
import DockArea from './DockArea.vue';
import NavNotices from './NavNotices.vue';
import { listWorktreeMarkdown, type MarkdownEntry } from './markdownFiles';
import { provideQuickOpen } from './quickOpen';
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
  api.addPanel({ id: NAVIGATOR_PANEL, component: 'navigator', title: t('agents.layouts.navigator'), initialWidth: NAVIGATOR_WIDTH });
}

function showNavigator() {
  const api = dock.value;
  if (!api) return;
  const existing = api.getPanel(NAVIGATOR_PANEL);
  if (existing) existing.api.setActive();
  else api.addPanel({ id: NAVIGATOR_PANEL, component: 'navigator', title: t('agents.layouts.navigator'), initialWidth: NAVIGATOR_WIDTH, position: { direction: 'left' } });
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

const switcherFiles = shallowRef<MarkdownEntry[]>([]);

watch(switching, async (on) => {
  switcherFiles.value = [];
  const worktree = state.activeWorktree.value;
  if (!on || !worktree) return;
  try {
    const files = await listWorktreeMarkdown(worktree.path);
    if (switching.value) switcherFiles.value = files;
  } catch {
    switcherFiles.value = [];
  }
});

function pick(session: SessionNode) {
  switching.value = false;
  state.open(session);
}

function pickFile(path: string) {
  switching.value = false;
  state.openFile(path, { worktreeId: state.activeWorktree.value?.id });
}

let stopQuickOpen = () => {};

onMounted(() => {
  stopQuickOpen = provideQuickOpen(() => {
    if (state.visible.value) switching.value = !switching.value;
  });
});
onBeforeUnmount(() => stopQuickOpen());
</script>

<template>
  <div class="layout">
    <div class="toolbar">
      <button class="tool" :title="t('agents.layouts.showNavigator')" data-testid="navigator-show" @click="showNavigator"><PanelLeft :size="14" />{{ t('agents.layouts.navigator') }}</button>
      <button class="tool" :title="t('agents.layouts.floatNavigatorTitle')" data-testid="navigator-float" @click="floatNavigator"><SquareArrowOutUpRight :size="14" />{{ t('agents.layouts.floating') }}</button>
      <button class="tool" :title="t('agents.layouts.quickOpenTitle')" data-testid="quick-switch-open" @click="switching = true"><Search :size="14" />{{ t('agents.layouts.quickOpen') }}<kbd>Ctrl+P</kbd></button>
      <span class="spacer" />
      <span class="tip">{{ t('agents.layouts.dragTip') }}</span>
      <button class="tool" :title="t('agents.metrics.open')" data-testid="open-metrics" @click="state.openMetrics()">{{ t('agents.metrics.title') }}</button>
    </div>
    <DockArea :active-key="activeKey" :paths="paths" group-labels @ready="state.onReady" @open-file="(path, hash, from) => state.openFile(path, { hash, from })">
      <template #navigator>
        <aside v-auto-animate class="navigator" :aria-label="t('agents.layouts.pacerSessions')" data-testid="navigator">
          <NavNotices />
          <section v-for="workspace in pacer?.tree.value ?? []" :key="workspace.id" v-auto-animate class="workspace">
            <div class="ws-head">
              <Layers :size="14" class="icon" />
              <h2 class="ws-name">{{ workspace.name }}</h2>
              <WorkspaceActions :workspace-id="workspace.id" />
            </div>
            <div v-for="project in workspace.projects" :key="project.id" v-auto-animate class="project">
              <div class="project-head">
                <Folder :size="14" class="icon" />
                <h3 class="project-name" :title="project.name">{{ project.name }}</h3>
                <ProjectActions :project-id="project.id" @resumed="state.openWhenListed({ Agent: $event })" @create="state.creatingIn.value = $event" />
              </div>
              <WorktreeBlock
                v-for="worktree in project.worktrees"
                :key="worktree.id"
                :worktree="worktree"
                :active-key="activeKey"
                @open="state.open"
                @create="state.creatingIn.value = worktree.id"
                @open-file="state.openFile($event, { worktreeId: worktree.id })"
              />
            </div>
            <ProjectActions :workspace-id="workspace.id" />
          </section>
          <p v-if="pacer?.connection.value.state === 'connected' && !pacer.sessions.value.size" class="empty">{{ t('agents.layouts.noSessions') }}</p>
          <WorkspaceActions class="new-workspace" />
        </aside>
      </template>
    </DockArea>
    <Transition name="qs">
      <QuickSwitcher v-if="switching" :entries="entries" :files="switcherFiles" :file-scope="state.activeWorktree.value?.branch" @select="pick" @select-file="pickFile" @close="switching = false" />
    </Transition>
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
