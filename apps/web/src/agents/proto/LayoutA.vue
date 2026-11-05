<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef, watch } from 'vue';
import { Archive, ChevronRight, Folder, FolderOpen, GitBranch, Layers, Plus } from 'lucide-vue-next';
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
import NavNotices from './NavNotices.vue';
import SidePane from '../../SidePane.vue';
import { rollupOf, workspaceSessions, type Rollup } from './nav';
import { useAgentsDock } from './useAgentsDock';

const EXPANDED_KEY = 'veyra.agents.explorer.expanded';

type Base = { id: string; depth: number; parent: string | null };
type GroupKind = 'workspace' | 'project' | 'worktree' | 'archive';
type GroupRow = Base & { kind: GroupKind; label: string; entityId: string; expanded: boolean; rollup: Rollup; archived: number; path?: string };
type Row =
  | GroupRow
  | (Base & { kind: 'session'; session: SessionNode })
  | (Base & { kind: 'archived'; session: SessionNode })
  | (Base & { kind: 'add-project'; workspaceId: string })
  | (Base & { kind: 'add-workspace' });

const state = useAgentsDock();
const { pacer, activeKey, paths } = state;

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
        out.push({ kind: 'worktree', id: wtId, depth: 2, parent: prId, label: worktree.branch, path: worktree.path, entityId: worktree.id, expanded: open.has(wtId), rollup: rollupOf(worktree.sessions), archived: worktree.archived.length });
        if (!open.has(wtId)) continue;
        for (const session of worktree.sessions) out.push({ kind: 'session', id: session.key, depth: 3, parent: wtId, session });
        if (worktree.archived.length) {
          const arId = `ar:${worktree.id}`;
          out.push({ kind: 'archive', id: arId, depth: 3, parent: wtId, label: 'Архив', entityId: worktree.id, expanded: open.has(arId), rollup: rollupOf([]), archived: worktree.archived.length });
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
  <div class="layout">
    <SidePane storage-key="veyra.agents.a.width" :default-width="300">
      <div class="head">
        <span>Проводник</span>
        <button class="text-button" title="Использование памяти" data-testid="open-metrics" @click="state.openMetrics()">Память</button>
      </div>
      <div class="scroll">
        <NavNotices />
        <div ref="tree" class="tree" role="tree" tabindex="0" aria-label="Сессии pacer" data-testid="explorer-tree" @keydown="onKey">
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
                <ChevronRight :size="14" class="chevron" :data-open="row.expanded" />
                <Layers v-if="row.kind === 'workspace'" :size="14" class="kind-icon" />
                <component :is="row.expanded ? FolderOpen : Folder" v-else-if="row.kind === 'project'" :size="14" class="kind-icon" />
                <GitBranch v-else-if="row.kind === 'worktree'" :size="14" class="kind-icon" />
                <Archive v-else :size="14" class="kind-icon" />
                <span class="label" :data-kind="row.kind">{{ row.label }}</span>
                <template v-if="!row.expanded && (row.kind === 'archive' || row.rollup.total)">
                  <span class="count" :title="row.kind === 'archive' ? 'в архиве' : 'работает / всего'" data-testid="node-count">{{ countOf(row) }}</span>
                  <StatusDot v-if="row.kind !== 'archive'" :status="row.rollup.status" :unseen="row.rollup.unseen" />
                </template>
              </button>
              <template v-if="row.kind === 'worktree'">
                <WorktreeBadge v-if="row.path" :path="row.path" :branch="row.label" @open="openReview" />
                <button class="add" title="Новая сессия" aria-label="Новая сессия" data-testid="worktree-new-session" @click="state.creatingIn.value = row.entityId"><Plus :size="14" /></button>
              </template>
              <WorkspaceActions v-else-if="row.kind === 'workspace'" class="node-actions" :workspace-id="row.entityId" />
              <ProjectActions v-else-if="row.kind === 'project'" class="node-actions" :project-id="row.entityId" @resumed="state.openWhenListed({ Agent: $event })" />
            </div>
          </template>
          <p v-if="pacer?.connection.value.state === 'connected' && !pacer.sessions.value.size" class="empty">Нет сессий</p>
        </div>
      </div>
    </SidePane>
    <DockArea :active-key="activeKey" :paths="paths" @ready="state.onReady" />
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

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--vr-space-1) var(--vr-space-2) 0 var(--vr-space-4);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.text-button {
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
  transition: transform 0.1s;
}

.chevron[data-open='true'] {
  transform: rotate(90deg);
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
