<script setup lang="ts">
import { computed, shallowRef, type Directive } from 'vue';
import { Activity, FolderTree, MessageSquare, Plus, RefreshCw, Search, Shapes } from 'lucide-vue-next';
import { useClient, useClientState } from '@core/vue-bindings';
import ArchivedRow from '../agents/ArchivedRow.vue';
import ProjectActions from '../agents/manage/ProjectActions.vue';
import WorkspaceActions from '../agents/manage/WorkspaceActions.vue';
import MarkdownPicker from '../agents/proto/MarkdownPicker.vue';
import NavNotices from '../agents/proto/NavNotices.vue';
import SessionRow from '../agents/SessionRow.vue';
import StatusDot from '../agents/StatusDot.vue';
import TypeIcon from '../atlassian/TypeIcon.vue';
import type { InboxItem } from '../connectors/api';
import { numberLabel, pipelineLabel } from '../forge/format';
import PipelineIcon from '../forge/PipelineIcon.vue';
import WorktreeBadge from '../forge/WorktreeBadge.vue';
import { t } from '../i18n';
import { openTopic, view } from '../nav';
import ExplorerTree from '../tree/ExplorerTree.vue';
import { useDock } from './dock';
import RepoActions from './RepoActions.vue';
import { treeMode, treeQuery, useWorkTree } from './useWorkTree';

withDefaults(defineProps<{ withFilter?: boolean }>(), { withFilter: false });

const dock = useDock();
const client = useClient();
const state = useClientState();
const { pacer, nodes, refresh, loading, errors } = useWorkTree();

const filtering = computed(() => treeQuery.value.trim().length > 0);
const selected = computed(() => (view.value.kind === 'topic' ? `topic:${view.value.id}` : dock.activeKey.value));
const vFocus: Directive<HTMLElement> = { mounted: (el) => el.focus() };
const creatingTopic = shallowRef(false);
const topicName = shallowRef('');
const topicError = shallowRef('');

const shownTitle =(item: InboxItem) => (item.draft ? item.title.replace(/^(draft:|\[draft\]|\(draft\)|wip:)\s*/i, '') : item.title);

async function createTopic() {
  const name = topicName.value.trim();
  if (!name) return;
  topicError.value = '';
  try {
    const topic = await client.createTopic(name);
    topicName.value = '';
    creatingTopic.value = false;
    openTopic(topic.id);
  } catch (e) {
    topicError.value = e instanceof Error ? e.message : String(e);
  }
}
</script>

<template>
  <section class="project-tree" data-testid="project-tree">
    <label v-if="withFilter" class="filter">
      <Search :size="14" />
      <input v-model="treeQuery" :placeholder="t('workbench.filter')" :aria-label="t('workbench.filter')" data-testid="tree-filter" @keydown.esc="treeQuery = ''" />
    </label>
    <div class="head">
      <div class="modes" role="radiogroup" :aria-label="t('workbench.grouping')">
        <button role="radio" :aria-checked="treeMode === 'project'" :aria-label="t('workbench.byProject')" :title="t('workbench.byProject')" data-testid="tree-mode-project" @click="treeMode = 'project'">
          <FolderTree :size="14" />
        </button>
        <button role="radio" :aria-checked="treeMode === 'type'" :aria-label="t('workbench.byType')" :title="t('workbench.byType')" data-testid="tree-mode-type" @click="treeMode = 'type'">
          <Shapes :size="14" />
        </button>
      </div>
      <span class="head-actions">
        <button class="icon" :title="t('agents.layouts.quickOpenTitle')" :aria-label="t('agents.layouts.quickOpen')" data-testid="quick-switch-open" @click="dock.switching.value = true"><Search :size="14" /></button>
        <button v-if="pacer" class="icon" :title="t('agents.metrics.open')" :aria-label="t('agents.metrics.title')" data-testid="open-metrics" @click="dock.openMetrics()"><Activity :size="14" /></button>
        <button class="icon" :title="t('workbench.refresh')" :aria-label="t('workbench.refresh')" :data-loading="loading" data-testid="tree-refresh" @click="refresh"><RefreshCw :size="14" :class="{ spin: loading }" /></button>
      </span>
    </div>
    <div class="scroll">
      <NavNotices />
      <p v-for="error in errors" :key="`${error.connector}:${error.message}`" class="strip" role="alert" data-testid="tree-error" :title="error.message">{{ error.connector }}: {{ error.message }}</p>
      <ExplorerTree
        :nodes="nodes"
        :storage-key="`veyra.tree.expanded.${treeMode}`"
        :label="t('workbench.tree')"
        :selected="selected"
        :reveal-all="filtering"
        test-id="explorer-tree"
        @open="dock.openLeaf"
      >
        <template #rollup="{ node }">
          <StatusDot v-if="node.data?.rollup?.total" :status="node.data.rollup.status" :unseen="node.data.rollup.unseen" />
          <PipelineIcon v-if="node.data?.pipeline && node.data.pipeline !== 'none'" :status="node.data.pipeline" :size="12" :title="pipelineLabel(node.data.pipeline)" />
        </template>
        <template #leaf="{ node, data }">
          <span v-if="data?.item" class="line" data-testid="tree-mr-row" :data-mr="`${data.item.project}!${data.item.number}`">
            <span class="key">{{ numberLabel(data.item) }}</span>
            <span class="title"><span v-if="data.item.draft" class="draft">{{ t('forge.draft') }}</span>{{ shownTitle(data.item) }}</span>
            <span v-if="data.item.unresolved_threads" class="threads" :title="t('forge.unresolvedThreads', { count: data.item.unresolved_threads })"><MessageSquare :size="12" />{{ data.item.unresolved_threads }}</span>
            <PipelineIcon v-if="data.item.pipeline !== 'none'" :status="data.item.pipeline" :size="12" :title="pipelineLabel(data.item.pipeline)" />
          </span>
          <span v-else-if="data?.issue" class="line" data-testid="tree-issue-row">
            <TypeIcon :type="data.issue.issue_type" />
            <span class="key">{{ data.issue.key }}</span>
            <span class="title">{{ data.issue.summary }}</span>
          </span>
          <span v-else-if="data?.task" class="line" data-testid="tree-task-row" :data-unread="state.unread.has(data.task.id)">
            <span class="key">{{ data.task.externalRef }}</span>
            <span class="title">{{ data.task.title }}</span>
            <span v-if="state.unread.has(data.task.id)" class="unread-dot" />
          </span>
          <span v-else class="line" :data-unread="data?.topic ? state.unread.has(data.topic.id) : undefined">
            <span class="title">{{ node.label }}</span>
            <span v-if="data?.topic && state.unread.has(data.topic.id)" class="unread-dot" />
          </span>
        </template>
        <template #row="{ node }">
          <SessionRow v-if="node.kind === 'session' && node.data?.session" :session="node.data.session" :active="node.id === dock.activeKey.value" @open="dock.openSession(node.data.session)" />
          <ArchivedRow v-else-if="node.kind === 'archived' && node.data?.session" :session="node.data.session" />
          <div v-else-if="node.kind === 'add-project' && node.data?.workspaceId" class="extra"><ProjectActions :workspace-id="node.data.workspaceId" /></div>
          <div v-else-if="node.kind === 'add-workspace'" class="extra new-workspace"><WorkspaceActions /></div>
        </template>
        <template #badges="{ node }">
          <WorktreeBadge v-if="node.kind === 'worktree' && node.data?.worktree" :path="node.data.worktree.path" :branch="node.data.worktree.branch" :summary="node.data.worktreeMr" @open="(ref) => dock.openReview(ref)" />
        </template>
        <template #actions="{ node }">
          <template v-if="node.kind === 'worktree' && node.data?.worktree">
            <MarkdownPicker :root="node.data.worktree.path" @open="dock.openFile($event, { worktreeId: node.data.worktree.id })" />
            <button class="icon" :title="t('agents.newSession.action')" :aria-label="t('agents.newSession.action')" data-testid="worktree-new-session" @click="dock.startCreate(node.data.worktree.id)"><Plus :size="14" /></button>
          </template>
          <WorkspaceActions v-else-if="node.kind === 'workspace' && node.data?.workspaceId" :workspace-id="node.data.workspaceId" />
          <RepoActions v-else-if="node.kind === 'repo' && node.data?.repo" :repo="node.data.repo" />
          <ProjectActions
            v-else-if="node.kind === 'project' && node.data?.projectId && treeMode === 'project'"
            :project-id="node.data.projectId"
            @resumed="dock.openWhenListed({ Agent: $event })"
            @create="dock.startCreate($event)"
          />
          <template v-else-if="node.kind === 'topics'">
            <form v-if="creatingTopic" class="inline" @submit.prevent="createTopic">
              <input v-model="topicName" v-focus class="topic-input" :placeholder="t('workbench.topicName')" :aria-label="t('workbench.topicName')" data-testid="topic-name" @keydown.esc="creatingTopic = false" />
              <span v-if="topicError" class="error" role="alert">{{ topicError }}</span>
            </form>
            <button v-else class="icon" :title="t('workbench.newTopic')" :aria-label="t('workbench.newTopic')" data-testid="topic-add" @click="creatingTopic = true"><Plus :size="14" /></button>
          </template>
        </template>
        <template #empty>
          <p class="empty" data-testid="tree-empty">{{ filtering ? t('workbench.nothingFound') : t('workbench.empty') }}</p>
        </template>
      </ExplorerTree>
    </div>
  </section>
</template>

<style scoped>
.project-tree {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  font-size: var(--vr-font-size-sm);
}

.filter {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: var(--vr-size-row);
  margin: var(--vr-space-2) var(--vr-space-2) 0;
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-nav-muted);
  background: var(--vr-color-nav-hover);
  border-radius: var(--vr-radius-md);
}

.filter input {
  flex: 1;
  min-width: 0;
  font: inherit;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  outline: none;
}

.head {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-1) var(--vr-space-1) var(--vr-space-2);
}

.modes {
  display: inline-flex;
  padding: 2px;
  background: var(--vr-color-nav-hover);
  border-radius: var(--vr-radius-md);
}

.modes button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 20px;
  padding: 0;
  color: var(--vr-color-nav-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  transition: background var(--vr-motion-overlay-in, 0.12s) ease;
}

.modes button[aria-checked='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
  box-shadow: var(--vr-elevation-card);
}

.head-actions {
  display: inline-flex;
  align-items: center;
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

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.scroll {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 0 var(--vr-space-1) var(--vr-space-2);
}

.strip {
  margin: 0 0 var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-2);
  overflow: hidden;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-danger);
  white-space: nowrap;
  text-overflow: ellipsis;
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-sm);
}

.scroll :deep(.label[data-kind='worktree']) {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.scroll :deep(.label[data-kind='sessions']),
.scroll :deep(.label[data-kind='mrs']),
.scroll :deep(.label[data-kind='tasks']),
.scroll :deep(.label[data-kind='section']),
.scroll :deep(.label[data-kind='archive']) {
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-text-muted);
}

.scroll :deep(.node-actions:has(.inline)),
.extra :deep(.actions:has(.inline)) {
  flex: 1 0 100%;
  padding: 0 var(--vr-space-2) var(--vr-space-2) calc(var(--depth) * var(--indent) + 24px);
  opacity: 1;
}

.scroll :deep(.custom .row) {
  padding-left: var(--vr-space-2);
}

.scroll :deep(.custom .archived .line) {
  padding-left: var(--vr-space-2);
}

.extra {
  padding-left: 12px;
}

.new-workspace {
  margin-top: var(--vr-space-2);
  padding-left: var(--vr-space-1);
}

.line {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
}

.key {
  flex: none;
  font-size: var(--vr-font-size-2xs);
  font-variant-numeric: tabular-nums;
  color: var(--vr-color-text-muted);
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.line[data-unread='true'] .title {
  font-weight: 600;
}

.draft {
  margin-right: var(--vr-space-1);
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.threads {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 2px;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-conflict);
}

.unread-dot {
  flex: none;
  width: 8px;
  height: 8px;
  background: var(--vr-color-accent);
  border-radius: 50%;
}

.inline {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
}

.topic-input {
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  font: inherit;
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
  border: 0;
  border-radius: var(--vr-radius-md);
  outline: none;
}

.error {
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-danger);
}

.empty {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
