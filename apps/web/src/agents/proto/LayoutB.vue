<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { ChevronRight, Folder } from 'lucide-vue-next';
import { t } from '../../i18n';
import NewSessionForm from '../NewSessionForm.vue';
import StatusDot from '../StatusDot.vue';
import ProjectActions from '../manage/ProjectActions.vue';
import WorkspaceActions from '../manage/WorkspaceActions.vue';
import DockArea from './DockArea.vue';
import NavNotices from './NavNotices.vue';
import SidePane from '../../SidePane.vue';
import WorktreeBlock from './WorktreeBlock.vue';
import { rollupOf, workspaceSessions } from './nav';
import { useAgentsDock } from './useAgentsDock';

const SELECTED_KEY = 'veyra.agents.b.workspace';
const AVATARS = 7;

const state = useAgentsDock();
const { pacer, activeKey, activePath, paths } = state;

const storedId = shallowRef(localStorage.getItem(SELECTED_KEY));
const workspaces = computed(() => pacer?.tree.value ?? []);
const selected = computed(() => workspaces.value.find((workspace) => workspace.id === storedId.value) ?? workspaces.value[0]);

function select(id: string) {
  storedId.value = id;
  localStorage.setItem(SELECTED_KEY, id);
}

watch(activePath, (path) => {
  if (path) select(path.workspace.id);
});

const rail = computed(() =>
  workspaces.value.map((workspace) => ({
    id: workspace.id,
    name: workspace.name,
    initials: initialsOf(workspace.name),
    color: `var(--vr-palette-avatar-${(hash(workspace.id) % AVATARS) + 1})`,
    rollup: rollupOf(workspaceSessions(workspace)),
  })),
);

function hash(text: string): number {
  let value = 0;
  for (const char of text) value = (value * 31 + (char.codePointAt(0) ?? 0)) >>> 0;
  return value;
}

function initialsOf(name: string): string {
  const words = name.trim().split(/[\s_-]+/).filter(Boolean);
  const letters = words.length > 1 ? words.slice(0, 2).map((word) => [...word][0] ?? '') : [...(words[0] ?? '?')].slice(0, 2);
  return letters.join('').toUpperCase();
}

function projectRollup(projectId: string) {
  const project = selected.value?.projects.find((candidate) => candidate.id === projectId);
  return rollupOf(project?.worktrees.flatMap((worktree) => worktree.sessions) ?? []);
}
</script>

<template>
  <div class="layout">
    <nav class="rail" aria-label="Workspaces" data-testid="workspace-rail">
      <button
        v-for="item in rail"
        :key="item.id"
        class="avatar"
        :style="{ background: item.color }"
        :title="item.name"
        :aria-label="item.name"
        :aria-pressed="item.id === selected?.id"
        :data-selected="item.id === selected?.id"
        data-testid="workspace-avatar"
        :data-workspace-id="item.id"
        @click="select(item.id)"
      >
        {{ item.initials }}
        <span v-if="item.rollup.feedback" class="badge" :title="t('agents.layouts.waitingReply')" data-testid="workspace-badge">{{ item.rollup.feedback }}</span>
        <span v-else-if="item.rollup.unseen" class="badge unseen" :title="t('agents.layouts.unread')" data-testid="workspace-badge-unseen" />
      </button>
    </nav>
    <SidePane storage-key="veyra.agents.b.width" :default-width="300">
      <div class="head">
        <h2 class="title" data-testid="workspace-title">{{ selected?.name ?? t('agents.workspace.fallback') }}</h2>
        <WorkspaceActions v-if="selected" :key="selected.id" class="head-actions" :workspace-id="selected.id" />
        <button class="text-button" :title="t('agents.metrics.open')" data-testid="open-metrics" @click="state.openMetrics()">{{ t('agents.metrics.title') }}</button>
      </div>
      <div v-auto-animate class="scroll">
        <NavNotices />
        <template v-if="selected">
          <section v-for="project in selected.projects" :key="project.id" v-auto-animate class="card" data-testid="project-card">
            <div class="card-head">
              <Folder :size="14" class="folder" />
              <h3 class="card-title" :title="project.name">{{ project.name }}</h3>
              <span v-if="projectRollup(project.id).running" class="running" :title="t('agents.layouts.running')">{{ projectRollup(project.id).running }}</span>
              <ProjectActions :project-id="project.id" @resumed="state.openWhenListed({ Agent: $event })" />
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
          </section>
          <ProjectActions :workspace-id="selected.id" />
        </template>
        <p v-if="pacer?.connection.value.state === 'connected' && !pacer.sessions.value.size" class="empty">{{ t('agents.layouts.noSessions') }}</p>
        <WorkspaceActions class="new-workspace" />
      </div>
    </SidePane>
    <div class="main">
      <nav class="crumbs" :aria-label="t('agents.layouts.sessionPath')" data-testid="breadcrumb">
        <template v-if="activePath">
          <button class="crumb" data-testid="crumb-workspace" @click="select(activePath.workspace.id)">{{ activePath.workspace.name }}</button>
          <ChevronRight :size="12" class="sep" />
          <span class="crumb" data-testid="crumb-project">{{ activePath.project.name }}</span>
          <ChevronRight :size="12" class="sep" />
          <span class="crumb mono" data-testid="crumb-worktree">{{ activePath.worktree.branch }}</span>
          <ChevronRight :size="12" class="sep" />
          <StatusDot :status="activePath.session.status" :unseen="activePath.session.unseen" />
          <strong class="crumb current" data-testid="crumb-session">{{ activePath.session.name }}</strong>
        </template>
        <span v-else class="hint">{{ t('agents.layouts.pathHint') }}</span>
      </nav>
      <DockArea :active-key="activeKey" :paths="paths" @ready="state.onReady" @open-file="(path, hash, from) => state.openFile(path, { hash, from })" />
    </div>
    <NewSessionForm v-if="state.creatingWorktree.value" :key="state.creatingWorktree.value.id" :worktree="state.creatingWorktree.value" @close="state.creatingIn.value = null" @created="state.onCreated" />
  </div>
</template>

<style scoped>
.layout {
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.rail {
  display: flex;
  flex: none;
  flex-direction: column;
  align-items: center;
  gap: var(--vr-space-2);
  width: 52px;
  padding: var(--vr-space-3) 0;
  overflow-y: auto;
  background: var(--vr-color-surface-muted);
  border-right: 1px solid var(--vr-color-border);
}

.avatar {
  position: relative;
  flex: none;
  width: 36px;
  height: 36px;
  padding: 0;
  font: inherit;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
  color: var(--vr-color-accent-contrast);
  border: 2px solid transparent;
  border-radius: var(--vr-radius-lg);
  opacity: 0.7;
}

.avatar:hover {
  opacity: 1;
}

.avatar[data-selected='true'] {
  opacity: 1;
  border-color: var(--vr-color-text);
}

.badge {
  position: absolute;
  top: -5px;
  right: -5px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  font-size: var(--vr-font-size-2xs);
  line-height: 16px;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-danger);
  border: 2px solid var(--vr-color-surface-muted);
  box-sizing: content-box;
  border-radius: var(--vr-radius-full);
}

.badge.unseen {
  min-width: 8px;
  height: 8px;
  padding: 0;
  background: var(--vr-color-success);
}

.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  padding: var(--vr-space-3) var(--vr-space-2) var(--vr-space-1) var(--vr-space-4);
}

.title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-lg);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.head-actions {
  flex: none;
}

.text-button {
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.text-button:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-2);
}

.card {
  margin-bottom: var(--vr-space-3);
  padding: var(--vr-space-2);
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
}

.card-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  padding-bottom: var(--vr-space-1);
  margin-bottom: var(--vr-space-1);
  border-bottom: 1px solid var(--vr-color-border);
}

.folder {
  flex: none;
  margin-left: var(--vr-space-1);
  color: var(--vr-color-text-muted);
}

.card-title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-base);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.running {
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
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

.main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.crumbs {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: 32px;
  padding: 0 var(--vr-space-4);
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  color: var(--vr-color-text-muted);
  background: var(--vr-color-surface);
  border-bottom: 1px solid var(--vr-color-border);
}

button.crumb {
  padding: 0;
  font: inherit;
  color: inherit;
  background: none;
  border: 0;
}

button.crumb:hover {
  color: var(--vr-color-text);
  text-decoration: underline;
}

.sep {
  flex: none;
}

.mono {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
}

.current {
  color: var(--vr-color-text);
}

.hint {
  color: var(--vr-color-text-muted);
}
</style>
