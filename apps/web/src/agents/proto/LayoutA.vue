<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { Archive, ChevronRight, Folder, FolderOpen, GitBranch, House, Layers, Plus, Search } from 'lucide-vue-next';
import { t } from '../../i18n';
import WorktreeBadge from '../../forge/WorktreeBadge.vue';
import { openReview } from '../../nav';
import type { SessionNode } from '../pacer';
import ArchivedRow from '../ArchivedRow.vue';
import NewSessionForm from '../NewSessionForm.vue';
import SessionRow from '../SessionRow.vue';
import StatusDot from '../StatusDot.vue';
import ProjectActions from '../manage/ProjectActions.vue';
import WorkspaceActions from '../manage/WorkspaceActions.vue';
import DockArea from './DockArea.vue';
import MarkdownPicker from './MarkdownPicker.vue';
import NavNotices from './NavNotices.vue';
import QuickSwitcher from './QuickSwitcher.vue';
import SidePane from '../../SidePane.vue';
import BackBar from '../../mobile/BackBar.vue';
import TerminalKeys from '../../mobile/TerminalKeys.vue';
import { useIsMobile } from '../../mobile/useIsMobile';
import { listWorktreeMarkdown, type MarkdownEntry } from './markdownFiles';
import { rollupOf, workspaceSessions, type Rollup } from './nav';
import { provideQuickOpen } from './quickOpen';
import { useAgentsDock } from './useAgentsDock';

const EXPANDED_KEY = 'veyra.agents.explorer.expanded';

type Base = { id: string; depth: number; parent: string | null };
type GroupKind = 'workspace' | 'project' | 'worktree' | 'archive';
type GroupRow = Base & { kind: GroupKind; label: string; entityId: string; expanded: boolean; rollup: Rollup; archived: number; path?: string; main?: boolean };
type Row =
  | GroupRow
  | (Base & { kind: 'session'; session: SessionNode })
  | (Base & { kind: 'archived'; session: SessionNode })
  | (Base & { kind: 'add-project'; workspaceId: string })
  | (Base & { kind: 'add-workspace' });

const state = useAgentsDock({
  position: (session, api, paths) => {
    const worktree = paths.get(session.key)?.worktree.id;
    const sibling = api.panels.find((panel) => panel.id !== session.key && paths.get(panel.id)?.worktree.id === worktree);
    if (sibling) return { referencePanel: sibling.id, direction: 'within' };
    const lastSession = api.panels.filter((panel) => paths.has(panel.id)).at(-1);
    if (lastSession) return { referencePanel: lastSession.id, direction: 'right' };
    return undefined;
  },
});
const { pacer, activeKey, paths } = state;
const mobile = useIsMobile();
const mobileDetail = shallowRef(false);
const activeSession = computed(() => (activeKey.value ? pacer?.sessions.value.get(activeKey.value) : undefined));

const switching = shallowRef(false);
const entries = computed(() => [...paths.value.values()]);
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

function openTreeFile(path: string, worktreeId: string) {
  state.openFile(path, { worktreeId });
  mobileDetail.value = mobile.value;
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

const tree = useTemplateRef<HTMLElement>('tree');
const cursor = shallowRef<string | null>(null);

function readExpanded(): Set<string> | null {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(EXPANDED_KEY) ?? 'null');
    return Array.isArray(raw) ? new Set(raw.filter((item): item is string => typeof item === 'string')) : null;
  } catch {
    return null;
  }
}

const initial = readExpanded();
const expanded = shallowRef<ReadonlySet<string>>(initial ?? new Set());
let seeded = initial !== null;

function setExpanded(next: Set<string>) {
  expanded.value = next;
  localStorage.setItem(EXPANDED_KEY, JSON.stringify([...next]));
}

watch(
  () => pacer?.tree.value,
  (workspaces) => {
    if (seeded || !workspaces?.length) return;
    seeded = true;
    const all = new Set<string>();
    for (const workspace of workspaces) {
      all.add(`ws:${workspace.id}`);
      for (const project of workspace.projects) {
        all.add(`pr:${project.id}`);
        for (const worktree of project.worktrees) all.add(`wt:${worktree.id}`);
      }
    }
    setExpanded(all);
  },
  { immediate: true },
);

const rows = computed<Row[]>(() => {
  const open = expanded.value;
  const out: Row[] = [];
  for (const workspace of pacer?.tree.value ?? []) {
    const wsId = `ws:${workspace.id}`;
    out.push({ kind: 'workspace', id: wsId, depth: 0, parent: null, label: workspace.name, entityId: workspace.id, expanded: open.has(wsId), rollup: rollupOf(workspaceSessions(workspace)), archived: 0 });
    if (!open.has(wsId)) continue;
    for (const project of workspace.projects) {
      const prId = `pr:${project.id}`;
      out.push({
        kind: 'project',
        id: prId,
        depth: 1,
        parent: wsId,
        label: project.name,
        entityId: project.id,
        expanded: open.has(prId),
        rollup: rollupOf(project.worktrees.flatMap((worktree) => worktree.sessions)),
        archived: 0,
      });
      if (!open.has(prId)) continue;
      for (const worktree of project.worktrees) {
        const wtId = `wt:${worktree.id}`;
        out.push({ kind: 'worktree', id: wtId, depth: 2, parent: prId, label: worktree.branch, path: worktree.path, main: worktree.main, entityId: worktree.id, expanded: open.has(wtId), rollup: rollupOf(worktree.sessions), archived: worktree.archived.length });
        if (!open.has(wtId)) continue;
        for (const session of worktree.sessions) out.push({ kind: 'session', id: session.key, depth: 3, parent: wtId, session });
        if (worktree.archived.length) {
          const arId = `ar:${worktree.id}`;
          out.push({ kind: 'archive', id: arId, depth: 3, parent: wtId, label: t('agents.layouts.archive'), entityId: worktree.id, expanded: open.has(arId), rollup: rollupOf([]), archived: worktree.archived.length });
          if (open.has(arId)) for (const session of worktree.archived) out.push({ kind: 'archived', id: `arch:${session.key}`, depth: 4, parent: arId, session });
        }
      }
    }
    out.push({ kind: 'add-project', id: `add-project:${workspace.id}`, depth: 1, parent: wsId, workspaceId: workspace.id });
  }
  out.push({ kind: 'add-workspace', id: 'add-workspace', depth: 0, parent: null });
  return out;
});

const navRows = computed(() => rows.value.filter((row) => row.kind !== 'add-project' && row.kind !== 'add-workspace'));

function isGroup(row: Row): row is GroupRow {
  return row.kind === 'workspace' || row.kind === 'project' || row.kind === 'worktree' || row.kind === 'archive';
}

function toggle(id: string, force?: boolean) {
  const next = new Set(expanded.value);
  const want = force ?? !next.has(id);
  if (want) next.add(id);
  else next.delete(id);
  setExpanded(next);
}

function setCursor(id: string) {
  cursor.value = id;
  void nextTick(() => {
    const target = Array.from(tree.value?.querySelectorAll<HTMLElement>('[data-row-id]') ?? []).find((element) => element.dataset.rowId === id);
    target?.scrollIntoView({ block: 'nearest' });
  });
}

function onGroupClick(row: GroupRow) {
  setCursor(row.id);
  toggle(row.id);
  tree.value?.focus();
}

function openSession(session: SessionNode) {
  setCursor(session.key);
  state.open(session);
  mobileDetail.value = mobile.value;
  tree.value?.focus();
}

watch(activeKey, (key) => {
  const path = key ? paths.value.get(key) : undefined;
  if (!key || !path) return;
  const next = new Set(expanded.value);
  next.add(`ws:${path.workspace.id}`);
  next.add(`pr:${path.project.id}`);
  next.add(`wt:${path.worktree.id}`);
  setExpanded(next);
  setCursor(key);
});

function onKey(event: KeyboardEvent) {
  const target = event.target;
  if (target instanceof HTMLElement && target.closest('input, textarea, [contenteditable]')) return;
  const list = navRows.value;
  const index = list.findIndex((row) => row.id === cursor.value);
  const current = index >= 0 ? list[index] : undefined;
  const move = (to: number) => {
    const row = list[Math.max(0, Math.min(list.length - 1, to))];
    if (row) setCursor(row.id);
  };
  switch (event.key) {
    case 'ArrowDown':
      move(index + 1);
      break;
    case 'ArrowUp':
      move(index < 0 ? 0 : index - 1);
      break;
    case 'Home':
      move(0);
      break;
    case 'End':
      move(list.length - 1);
      break;
    case 'ArrowRight':
      if (!current || !isGroup(current)) return;
      if (!current.expanded) toggle(current.id, true);
      else move(index + 1);
      break;
    case 'ArrowLeft': {
      if (!current) return;
      if (isGroup(current) && current.expanded) toggle(current.id, false);
      else if (current.parent) setCursor(current.parent);
      break;
    }
    case 'Enter':
      if (target !== tree.value || !current) return;
      if (current.kind === 'session') state.open(current.session);
      else if (isGroup(current)) toggle(current.id);
      break;
    default:
      return;
  }
  event.preventDefault();
}

function countOf(row: GroupRow): string {
  return row.kind === 'archive' ? String(row.archived) : `${row.rollup.running}/${row.rollup.total}`;
}
</script>

<template>
  <div class="layout" :data-mobile="mobile" :data-detail="mobileDetail">
    <SidePane class="tree-pane" storage-key="veyra.agents.a.width" :default-width="300">
      <div class="head">
        <span>{{ t('agents.layouts.explorer') }}</span>
        <span class="head-actions">
          <button class="text-button" :title="t('agents.layouts.quickOpenTitle')" data-testid="quick-switch-open" @click="switching = true"><Search :size="14" />{{ t('agents.layouts.quickOpen') }}<kbd>Ctrl+P</kbd></button>
          <button class="text-button" :title="t('agents.metrics.open')" data-testid="open-metrics" @click="state.openMetrics()">{{ t('agents.metrics.title') }}</button>
        </span>
      </div>
      <div class="scroll">
        <NavNotices />
        <div ref="tree" v-auto-animate class="tree" role="tree" tabindex="0" :aria-label="t('agents.layouts.pacerSessions')" data-testid="explorer-tree" @keydown="onKey">
          <template v-for="row in rows" :key="row.id">
            <div v-if="row.kind === 'add-project'" class="node extra" :style="{ '--depth': row.depth }">
              <i v-for="level in row.depth" :key="level" class="guide" :style="{ '--level': level - 1 }" />
              <ProjectActions :workspace-id="row.workspaceId" />
            </div>
            <div v-else-if="row.kind === 'add-workspace'" class="node extra new-workspace" :style="{ '--depth': row.depth }">
              <WorkspaceActions />
            </div>
            <div
              v-else-if="row.kind === 'session' || row.kind === 'archived'"
              class="node leaf"
              role="treeitem"
              :aria-level="row.depth + 1"
              :aria-selected="row.id === cursor"
              :data-row-id="row.id"
              :data-kind="row.kind"
              :data-cursor="row.id === cursor"
              :style="{ '--depth': row.depth }"
            >
              <i v-for="level in row.depth" :key="level" class="guide" :style="{ '--level': level - 1 }" />
              <SessionRow v-if="row.kind === 'session'" :session="row.session" :active="row.session.key === activeKey" @open="openSession(row.session)" />
              <ArchivedRow v-else :session="row.session" />
            </div>
            <div
              v-else
              class="node group"
              role="treeitem"
              :aria-level="row.depth + 1"
              :aria-expanded="row.expanded"
              :aria-selected="row.id === cursor"
              :data-row-id="row.id"
              :data-kind="row.kind"
              :data-cursor="row.id === cursor"
              :style="{ '--depth': row.depth }"
            >
              <i v-for="level in row.depth" :key="level" class="guide" :style="{ '--level': level - 1 }" />
              <button class="toggle" :title="row.label" :data-testid="`node-${row.kind}`" @click="onGroupClick(row)">
                <ChevronRight :size="14" class="chevron vr-chevron" :data-open="row.expanded" />
                <Layers v-if="row.kind === 'workspace'" :size="14" class="kind-icon" />
                <component :is="row.expanded ? FolderOpen : Folder" v-else-if="row.kind === 'project'" :size="14" class="kind-icon" />
                <House v-else-if="row.kind === 'worktree' && row.main" :size="14" class="kind-icon" :aria-label="t('agents.worktree.mainCheckout')" />
                <GitBranch v-else-if="row.kind === 'worktree'" :size="14" class="kind-icon" />
                <Archive v-else :size="14" class="kind-icon" />
                <span class="label" :data-kind="row.kind">{{ row.label }}</span>
                <template v-if="!row.expanded && (row.kind === 'archive' || row.rollup.total)">
                  <span class="count" :title="row.kind === 'archive' ? t('agents.layouts.archivedCount') : t('agents.layouts.runningTotal')" data-testid="node-count">{{ countOf(row) }}</span>
                  <StatusDot v-if="row.kind !== 'archive'" :status="row.rollup.status" :unseen="row.rollup.unseen" />
                </template>
              </button>
              <template v-if="row.kind === 'worktree'">
                <WorktreeBadge v-if="row.path" :path="row.path" :branch="row.label" @open="openReview" />
                <MarkdownPicker v-if="row.path" :root="row.path" @open="openTreeFile($event, row.entityId)" />
                <button class="add" :title="t('agents.newSession.action')" :aria-label="t('agents.newSession.action')" data-testid="worktree-new-session" @click="state.creatingIn.value = row.entityId"><Plus :size="14" /></button>
              </template>
              <WorkspaceActions v-else-if="row.kind === 'workspace'" class="node-actions" :workspace-id="row.entityId" />
              <ProjectActions v-else-if="row.kind === 'project'" class="node-actions" :project-id="row.entityId" @resumed="state.openWhenListed({ Agent: $event })" @create="state.creatingIn.value = $event" />
            </div>
          </template>
          <p v-if="pacer?.connection.value.state === 'connected' && !pacer.sessions.value.size" class="empty">{{ t('agents.layouts.noSessions') }}</p>
        </div>
      </div>
    </SidePane>
    <BackBar v-if="mobile && mobileDetail" :title="activeSession?.name" @back="mobileDetail = false" />
    <DockArea class="dock-slot" :active-key="activeKey" :paths="paths" group-labels @ready="state.onReady" @open-file="(path, hash, from) => state.openFile(path, { hash, from })" />
    <TerminalKeys v-if="mobile && mobileDetail && activeSession" :session="activeSession.ref" />
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
  min-width: 0;
  min-height: 0;
}

.layout[data-mobile='true'] {
  flex-direction: column;
}

.layout[data-mobile='true'] > .tree-pane {
  flex: 1;
  width: 100% !important;
  max-width: none;
  border-right: 0;
  min-height: 0;
}

.layout[data-mobile='true'] > .tree-pane :deep(.resize-handle) {
  display: none;
}

.layout[data-mobile='true'][data-detail='true'] > .tree-pane,
.layout[data-mobile='true'][data-detail='false'] > .dock-slot {
  display: none;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--vr-space-1) var(--vr-space-2) 0 var(--vr-space-4);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.head-actions {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
}

kbd {
  margin-left: var(--vr-space-1);
  padding: 0 var(--vr-space-1);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-sm);
}

.text-button {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 0 var(--vr-space-2);
  font: inherit;
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

.tree {
  --indent: 14px;
  outline: none;
}

.node {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  border-radius: var(--vr-radius-md);
}

.node[data-cursor='true'] {
  background: var(--vr-color-nav-hover);
}

.tree:focus-visible .node[data-cursor='true'] {
  box-shadow: inset 0 0 0 1px var(--vr-color-accent);
}

.guide {
  position: absolute;
  top: 0;
  bottom: 0;
  left: calc(var(--level) * var(--indent) + 13px);
  width: 1px;
  background: var(--vr-color-border);
  pointer-events: none;
}

.leaf {
  display: block;
  padding-left: calc(var(--depth) * var(--indent));
}

.leaf :deep(.row) {
  padding-left: var(--vr-space-2);
}

.leaf :deep(.archived .line) {
  padding-left: var(--vr-space-2);
}

.toggle {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-1) 0 calc(var(--depth) * var(--indent) + 6px);
  font: inherit;
  text-align: left;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.toggle:hover {
  background: var(--vr-color-nav-hover);
}

.chevron {
  flex: none;
  color: var(--vr-color-text-muted);
}

.kind-icon {
  flex: none;
  color: var(--vr-color-text-muted);
}

.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.label[data-kind='workspace'] {
  font-weight: 600;
}

.label[data-kind='project'] {
  font-weight: 500;
}

.label[data-kind='worktree'] {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.label[data-kind='archive'] {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.count {
  flex: none;
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.add {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  margin-right: var(--vr-space-1);
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  opacity: 0;
}

.node:hover .add,
.node[data-cursor='true'] .add,
.add:focus-visible {
  opacity: 1;
}

.add:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.node-actions {
  flex: none;
}

.node-actions:has(.inline),
.extra :deep(.actions:has(.inline)) {
  flex: 1 0 100%;
  padding: 0 var(--vr-space-2) var(--vr-space-2) calc(var(--depth) * var(--indent) + 24px);
}

.extra {
  padding-left: calc(var(--depth) * var(--indent) + 12px);
}

.new-workspace {
  margin-top: var(--vr-space-2);
  padding-left: var(--vr-space-1);
}

.empty {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}
</style>
