<script setup lang="ts">
import { computed, shallowRef, useTemplateRef } from 'vue';
import { Activity, Eye, GitPullRequest, ListTodo, MessageSquare, Plus, RefreshCw, Search, SquareTerminal, TriangleAlert } from 'lucide-vue-next';
import { VContextMenu } from '@ui/kit';
import ArchivedRow from '../agents/ArchivedRow.vue';
import { useExternalSessions } from '../agents/external';
import ExternalSessionRow from '../agents/ExternalSessionRow.vue';
import ProjectActions from '../agents/manage/ProjectActions.vue';
import WorkspaceActions from '../agents/manage/WorkspaceActions.vue';
import MarkdownPicker from '../agents/proto/MarkdownPicker.vue';
import NavNotices from '../agents/proto/NavNotices.vue';
import SessionRow from '../agents/SessionRow.vue';
import StatusDot from '../agents/StatusDot.vue';
import TypeIcon from '../atlassian/TypeIcon.vue';
import type { InboxItem } from '../connectors/api';
import ArchitectureAction from '../forge/ArchitectureAction.vue';
import { numberLabel, pipelineLabel } from '../forge/format';
import { useArchitecture } from '../forge/stratum';
import PipelineIcon from '../forge/PipelineIcon.vue';
import WorktreeBadge from '../forge/WorktreeBadge.vue';
import { t } from '../i18n';
import ExplorerTree from '../tree/ExplorerTree.vue';
import { useDock } from './dock';
import RepoActions from './RepoActions.vue';
import type { ReviewMeta } from './tree';
import { treeQuery, treeTab, useWorkTree } from './useWorkTree';

withDefaults(defineProps<{ withFilter?: boolean }>(), { withFilter: false });

const dock = useDock();
const { berth, nodes, tabs, activeTab, refresh, loading, errors } = useWorkTree();
const tabIcons = { sessions: SquareTerminal, mrs: GitPullRequest, reviews: Eye, tasks: ListTodo, all: SquareTerminal };

const filtering = computed(() => treeQuery.value.trim().length > 0);
const external = useExternalSessions();
const selected = computed(() => {
  const key = dock.activeKey.value;
  return key ? (external?.terminalOf.value.get(key) ?? key) : null;
});

const archMenu = useTemplateRef<InstanceType<typeof VContextMenu>>('archMenu');
const archItem = shallowRef<InboxItem | null>(null);
const { availability: archState } = useArchitecture(() => archItem.value ?? undefined);
const archItems = computed(() => {
  const item = archItem.value;
  const state = archState.value;
  if (!item || state.status === 'hidden') return [];
  const reason = state.status === 'disabled' ? state.reason : state.status === 'checking' ? t('forge.stratum.checking') : t('forge.stratum.title');
  return [{ key: 'architecture', label: t('forge.stratum.action'), title: reason, disabled: state.status !== 'ready', command: () => state.status === 'ready' && dock.openArchitecture(state.params, numberLabel(item)) }];
});

function showArchMenu(event: MouseEvent, item: InboxItem) {
  archItem.value = item;
  if (archState.value.status === 'hidden') return;
  event.preventDefault();
  archMenu.value?.show(event);
}

const jiraRefs = (review: ReviewMeta | undefined) => review?.refs.filter((ref) => !ref.startsWith('!')).join(', ') ?? '';
const shownTitle =(item: InboxItem) => (item.draft ? item.title.replace(/^(draft:|\[draft\]|\(draft\)|wip:)\s*/i, '') : item.title);
</script>

<template>
  <section class="project-tree" data-testid="project-tree">
    <label v-if="withFilter" class="filter">
      <Search :size="14" />
      <input v-model="treeQuery" :placeholder="t('workbench.filter')" :aria-label="t('workbench.filter')" data-testid="tree-filter" @keydown.esc="treeQuery = ''" />
    </label>
    <div class="head">
      <div class="tabs-box">
        <div class="tabs" role="tablist" :aria-label="t('workbench.tabs')" :data-n="tabs.length">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            class="tab"
            role="tab"
            :aria-selected="activeTab === tab.id"
            :aria-label="tab.label"
            :title="tab.title"
            :data-testid="`tree-tab-${tab.id}`"
            @click="treeTab = tab.id"
          >
            <component :is="tabIcons[tab.id]" :size="13" class="tab-icon" />
            <span class="tab-label">{{ tab.label }}</span>
            <span v-if="tab.count" class="badge" :data-tone="tab.tone" :data-testid="`tree-tab-count-${tab.id}`">{{ tab.count }}</span>
          </button>
        </div>
      </div>
      <span class="head-actions">
        <button class="icon" :title="t('agents.layouts.quickOpenTitle')" :aria-label="t('agents.layouts.quickOpen')" data-testid="quick-switch-open" @click="dock.switching.value = true"><Search :size="14" /></button>
        <button v-if="berth" class="icon" :title="t('agents.metrics.open')" :aria-label="t('agents.metrics.title')" data-testid="open-metrics" @click="dock.openMetrics()"><Activity :size="14" /></button>
        <button class="icon" :title="t('workbench.refresh')" :aria-label="t('workbench.refresh')" :data-loading="loading" data-testid="tree-refresh" @click="refresh"><RefreshCw :size="14" :class="{ spin: loading }" /></button>
      </span>
    </div>
    <div class="scroll">
      <NavNotices />
      <p v-for="error in errors" :key="`${error.connector}:${error.message}`" class="strip" role="alert" data-testid="tree-error" :title="error.message">{{ error.connector }}: {{ error.message }}</p>
      <ExplorerTree
        :nodes="nodes"
        :storage-key="`berth.tree.expanded.${activeTab}`"
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
          <span v-if="data?.item" class="line" data-testid="tree-mr-row" :data-mr="`${data.item.project}!${data.item.number}`" @contextmenu="showArchMenu($event, data.item)">
            <span class="key">{{ numberLabel(data.item) }}</span>
            <span v-if="jiraRefs(data.review)" class="refs" data-testid="tree-mr-refs">{{ jiraRefs(data.review) }}</span>
            <span class="title"><span v-if="data.item.draft" class="draft">{{ t('forge.draft') }}</span>{{ shownTitle(data.item) }}</span>
            <span v-if="data.review" class="verdict" :data-approved="data.review.approved" :title="data.review.approved ? t('workbench.review.approvedTitle') : t('workbench.review.pendingTitle')" data-testid="tree-mr-verdict">{{ data.review.approved ? t('workbench.review.approved') : t('workbench.review.pending') }}</span>
            <span v-if="data.item.unresolved_threads" class="threads" :title="t('forge.unresolvedThreads', { count: data.item.unresolved_threads })"><MessageSquare :size="12" />{{ data.item.unresolved_threads }}</span>
            <PipelineIcon v-if="data.item.pipeline !== 'none'" :status="data.item.pipeline" :size="12" :title="pipelineLabel(data.item.pipeline)" />
          </span>
          <span v-else-if="data?.issue" class="line" data-testid="tree-issue-row">
            <TypeIcon :type="data.issue.issue_type" />
            <span class="key">{{ data.issue.key }}</span>
            <span class="title">{{ data.issue.summary }}</span>
          </span>
          <span v-else class="line">
            <span class="title">{{ node.label }}</span>
          </span>
        </template>
        <template #row="{ node }">
          <SessionRow
            v-if="node.kind === 'session' && node.data?.session"
            :session="node.data.session"
            :linked="node.data.linked"
            :active="node.id === selected"
            @open="dock.openSession(node.data.session)"
            @open-conversation="node.data.linked && dock.openLinkedConversation(node.data.session, node.data.linked)"
          />
          <ExternalSessionRow v-else-if="node.kind === 'external' && node.data?.external" :session="node.data.external" :active="node.id === dock.activeKey.value" @open="dock.openExternal(node.data.external)" />
          <ArchivedRow v-else-if="node.kind === 'archived' && node.data?.session" :session="node.data.session" />
          <div v-else-if="node.kind === 'review-state' && node.data?.reviewState" class="review-state" :data-error="!!node.data.reviewState.error" :title="node.data.reviewState.error ?? undefined" role="status" data-testid="review-state">
            <template v-if="node.data.reviewState.error"><TriangleAlert :size="12" /><span class="review-text">{{ node.data.reviewState.error }}</span></template>
            <template v-else><RefreshCw :size="12" class="spin" /><span class="review-text">{{ node.label }}</span></template>
          </div>
          <ul v-else-if="node.kind === 'review-notes' && node.data?.notes" class="review-notes" :aria-label="node.label" data-testid="review-notes">
            <li v-for="(note, index) in node.data.notes" :key="index" :title="note.text"><b v-if="note.refs.length">{{ note.refs.join(', ') }}</b> {{ note.text }}</li>
          </ul>
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
          <ArchitectureAction v-else-if="node.data?.item" :item="node.data.item" variant="icon" />
          <ProjectActions
            v-else-if="node.kind === 'project' && node.data?.projectId"
            :project-id="node.data.projectId"
            @resumed="dock.openWhenListed({ Agent: $event })"
            @create="dock.startCreate($event)"
          />
        </template>
        <template #empty>
          <p class="empty" data-testid="tree-empty">{{ filtering ? t('workbench.nothingFound') : t('workbench.empty') }}</p>
        </template>
      </ExplorerTree>
    </div>
    <VContextMenu ref="archMenu" :model="archItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :title="item.title" data-testid="mr-menu-architecture">{{ item.label }}</a>
      </template>
    </VContextMenu>
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
  padding: var(--vr-space-1) var(--vr-space-1) var(--vr-space-1) var(--vr-space-1);
}

.tabs-box {
  flex: 1;
  min-width: 0;
  container: tabs / inline-size;
}

.tabs {
  display: flex;
  gap: 2px;
  padding: 2px;
  background: var(--vr-color-nav-hover);
  border-radius: var(--vr-radius-md);
}

.tab {
  display: inline-flex;
  flex: 1 1 auto;
  align-items: center;
  justify-content: center;
  gap: 3px;
  min-width: 0;
  height: 22px;
  padding: 0 2px;
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-nav-muted);
  white-space: nowrap;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  transition: background var(--vr-motion-overlay-in, 0.12s) ease;
}

.tab:hover {
  color: var(--vr-color-text);
}

.tab[aria-selected='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
  box-shadow: var(--vr-elevation-card);
}

.tab-icon {
  display: none;
  flex: none;
}

.tab-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.badge {
  flex: none;
  min-width: 14px;
  height: 14px;
  padding: 0 4px;
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  line-height: 14px;
  text-align: center;
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.badge[data-tone='accent'] {
  color: var(--vr-color-accent);
}

.badge[data-tone='alert'] {
  font-weight: 600;
  color: var(--vr-color-danger);
  background: color-mix(in srgb, var(--vr-color-danger) 20%, transparent);
}

@container tabs (max-width: 214px) {
  .tab-label {
    display: none;
  }

  .tab-icon {
    display: block;
  }
}

@container tabs (max-width: 262px) {
  .tabs[data-n='4'] .tab-label {
    display: none;
  }

  .tabs[data-n='4'] .tab-icon {
    display: block;
  }
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

.head-actions .icon {
  width: 20px;
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
.scroll :deep(.label[data-kind='archive']),
.scroll :deep(.label[data-kind='earlier']),
.scroll :deep(.label[data-kind='folders']) {
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

.refs {
  flex: none;
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-accent);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.verdict {
  flex: none;
  padding: 0 var(--vr-space-1);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.verdict[data-approved='true'] {
  color: var(--vr-color-success, var(--vr-color-accent));
}

.review-state {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-1) calc(var(--vr-space-2) + 14px);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.review-state[data-error='true'] {
  color: var(--vr-color-danger);
}

.review-text {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.review-notes {
  margin: 0;
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-1) calc(var(--vr-space-2) + 14px);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  list-style: none;
}

.review-notes li {
  display: -webkit-box;
  overflow: hidden;
  overflow-wrap: anywhere;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
}

.empty {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
