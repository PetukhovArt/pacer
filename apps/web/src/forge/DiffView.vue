<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch, type ShallowRef } from 'vue';
import { ChevronDown, ChevronRight, ChevronUp, Columns2, Folder, FolderOpen, Rows2, Search } from 'lucide-vue-next';
import { VInputText } from '@ui/kit';
import type { DiffFile, Discussion } from '../connectors/api';
import DiffFileBody from './DiffFileBody.vue';

const props = defineProps<{ files: DiffFile[]; discussions: Discussion[]; connector?: string }>();

type Mode = 'unified' | 'split';
type IndexSet = ShallowRef<ReadonlySet<number>>;
type TreeRow = { key: string; kind: 'dir' | 'file'; depth: number; name: string; path: string; index: number; additions: number; deletions: number; open: boolean };
type TreeNode = { name: string; path: string; dirs: Map<string, TreeNode>; files: number[] };
type Located = { discussion: Discussion; file: number; line: number };

const MODE_KEY = 'veyra.diff.mode';
const LINE_HEIGHT = 20;
const EMPTY: Discussion[] = [];

function storedMode(): Mode {
  try {
    return localStorage.getItem(MODE_KEY) === 'split' ? 'split' : 'unified';
  } catch {
    return 'unified';
  }
}

const mode = shallowRef<Mode>(storedMode());
const filter = shallowRef('');
const collapsed: IndexSet = shallowRef(new Set<number>());
const shown: IndexSet = shallowRef(new Set<number>());
const expanded: IndexSet = shallowRef(new Set<number>());
const closedDirs = shallowRef<ReadonlySet<string>>(new Set());
const activeIndex = shallowRef(0);
const threadCursor = shallowRef(-1);
const flashId = shallowRef<string | null>(null);

const root = useTemplateRef<HTMLElement>('root');
const scroller = useTemplateRef<HTMLElement>('scroller');
const fileEls: (HTMLElement | null)[] = [];
let observer: IntersectionObserver | undefined;
let flashTimer: ReturnType<typeof setTimeout> | undefined;
let frame = 0;

function setMode(next: Mode) {
  mode.value = next;
  try {
    localStorage.setItem(MODE_KEY, next);
  } catch {
    return;
  }
}

function toggle(set: IndexSet, index: number) {
  const next = new Set(set.value);
  if (!next.delete(index)) next.add(index);
  set.value = next;
}

function include(set: IndexSet, index: number) {
  if (!set.value.has(index)) set.value = new Set(set.value).add(index);
}

function exclude(set: IndexSet, index: number) {
  if (!set.value.has(index)) return;
  const next = new Set(set.value);
  next.delete(index);
  set.value = next;
}

const toggleCollapsed = (index: number) => toggle(collapsed, index);
const expandFile = (index: number) => include(expanded, index);

const totals = computed(() => props.files.reduce((acc, f) => ({ additions: acc.additions + f.additions, deletions: acc.deletions + f.deletions }), { additions: 0, deletions: 0 }));

const estimates = computed(() =>
  props.files.map((file) => {
    let count = 1;
    for (let at = file.patch.indexOf('\n'); at !== -1; at = file.patch.indexOf('\n', at + 1)) count++;
    return count * LINE_HEIGHT + 24;
  }),
);

const fileDiscussions = computed<Discussion[][]>(() => {
  const byPath = new Map<string, Discussion[]>();
  for (const d of props.discussions) {
    const path = d.position?.path;
    if (!path) continue;
    const list = byPath.get(path);
    if (list) list.push(d);
    else byPath.set(path, [d]);
  }
  return props.files.map((file) => {
    const current = byPath.get(file.new_path) ?? EMPTY;
    if (file.old_path === file.new_path) return current;
    const merged = [...current, ...(byPath.get(file.old_path) ?? EMPTY)];
    return merged.length ? merged : EMPTY;
  });
});

function note(file: DiffFile): string | null {
  if (file.binary) return 'Бинарный файл';
  if (file.too_large) return 'Файл слишком большой для показа, откройте его в браузере';
  if (!file.patch) return file.status === 'renamed' ? 'Файл переименован без изменений содержимого' : 'Нет изменений в содержимом';
  return null;
}

const STATUS_MARK: Record<DiffFile['status'], string> = { added: 'A', deleted: 'D', modified: 'M', renamed: 'R' };
const STATUS_TITLE: Record<DiffFile['status'], string> = { added: 'Добавлен', deleted: 'Удалён', modified: 'Изменён', renamed: 'Переименован' };

function buildTree(): TreeNode {
  const top: TreeNode = { name: '', path: '', dirs: new Map(), files: [] };
  props.files.forEach((file, index) => {
    let node = top;
    for (const part of file.new_path.split('/').slice(0, -1)) {
      let child = node.dirs.get(part);
      if (!child) {
        child = { name: part, path: node.path ? `${node.path}/${part}` : part, dirs: new Map(), files: [] };
        node.dirs.set(part, child);
      }
      node = child;
    }
    node.files.push(index);
  });
  return top;
}

function sumOf(node: TreeNode): { additions: number; deletions: number } {
  let additions = 0;
  let deletions = 0;
  for (const index of node.files) {
    additions += props.files[index]!.additions;
    deletions += props.files[index]!.deletions;
  }
  for (const child of node.dirs.values()) {
    const sum = sumOf(child);
    additions += sum.additions;
    deletions += sum.deletions;
  }
  return { additions, deletions };
}

const treeRows = computed<TreeRow[]>(() => {
  const query = filter.value.trim().toLowerCase();
  if (query) {
    return props.files
      .map((file, index) => ({ file, index }))
      .filter(({ file }) => file.new_path.toLowerCase().includes(query) || file.old_path.toLowerCase().includes(query))
      .map(({ file, index }) => ({ key: `f${index}`, kind: 'file' as const, depth: 0, name: file.new_path, path: file.new_path, index, additions: file.additions, deletions: file.deletions, open: false }));
  }
  const rows: TreeRow[] = [];
  const walk = (node: TreeNode, depth: number) => {
    for (const dir of [...node.dirs.values()].sort((a, b) => a.name.localeCompare(b.name))) {
      let tail = dir;
      let name = dir.name;
      while (tail.files.length === 0 && tail.dirs.size === 1) {
        tail = [...tail.dirs.values()][0]!;
        name = `${name}/${tail.name}`;
      }
      const open = !closedDirs.value.has(tail.path);
      const sum = sumOf(tail);
      rows.push({ key: `d${tail.path}`, kind: 'dir', depth, name, path: tail.path, index: -1, additions: sum.additions, deletions: sum.deletions, open });
      if (open) walk(tail, depth + 1);
    }
    const ordered = [...node.files].sort((a, b) => props.files[a]!.new_path.localeCompare(props.files[b]!.new_path));
    for (const index of ordered) {
      const file = props.files[index]!;
      rows.push({ key: `f${index}`, kind: 'file', depth, name: file.new_path.split('/').at(-1)!, path: file.new_path, index, additions: file.additions, deletions: file.deletions, open: false });
    }
  };
  walk(buildTree(), 0);
  return rows;
});

function toggleDir(row: TreeRow) {
  const next = new Set(closedDirs.value);
  if (!next.delete(row.path)) next.add(row.path);
  closedDirs.value = next;
}

function alignTo(index: number) {
  const el = fileEls[index];
  const host = scroller.value;
  if (el && host) host.scrollTo({ top: el.offsetTop });
}

async function scrollToFile(index: number) {
  if (index < 0 || index >= props.files.length) return;
  activeIndex.value = index;
  include(shown, index);
  alignTo(index);
  await nextTick();
  alignTo(index);
  requestAnimationFrame(() => alignTo(index));
}

function trackActive() {
  frame = 0;
  const host = scroller.value;
  if (!host) return;
  const edge = host.scrollTop + 8;
  let current = 0;
  for (let i = 0; i < props.files.length; i++) {
    const el = fileEls[i];
    if (!el) continue;
    if (el.offsetTop > edge) break;
    current = i;
  }
  activeIndex.value = current;
}

function onScroll() {
  if (!frame) frame = requestAnimationFrame(trackActive);
}

const unresolved = computed<Located[]>(() => {
  const located: Located[] = [];
  fileDiscussions.value.forEach((list, file) => {
    for (const d of list) {
      if (d.resolvable && !d.resolved) located.push({ discussion: d, file, line: d.position?.new_line ?? d.position?.old_line ?? 0 });
    }
  });
  return located.sort((a, b) => a.file - b.file || a.line - b.line);
});

function findThread(id: string): HTMLElement | null {
  return scroller.value?.querySelector<HTMLElement>(`[data-thread-id="${CSS.escape(id)}"]`) ?? null;
}

async function revealThread(id: string, fileIndex: number) {
  include(shown, fileIndex);
  exclude(collapsed, fileIndex);
  await nextTick();
  let el = findThread(id);
  if (!el) {
    include(expanded, fileIndex);
    await nextTick();
    el = findThread(id);
  }
  if (!el) {
    await scrollToFile(fileIndex);
    return;
  }
  activeIndex.value = fileIndex;
  el.scrollIntoView({ block: 'center' });
  flashId.value = id;
  clearTimeout(flashTimer);
  flashTimer = setTimeout(() => (flashId.value = null), 1500);
}

function stepThread(delta: 1 | -1) {
  const list = unresolved.value;
  if (!list.length) return;
  threadCursor.value = (threadCursor.value + delta + list.length) % list.length;
  const target = list[threadCursor.value]!;
  void revealThread(target.discussion.id, target.file);
}

function focusThread(id: string) {
  const index = fileDiscussions.value.findIndex((list) => list.some((d) => d.id === id));
  if (index >= 0) void revealThread(id, index);
}

function onKey(event: KeyboardEvent) {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
  if (event.key === 'j') void scrollToFile(Math.min(activeIndex.value + 1, props.files.length - 1));
  else if (event.key === 'k') void scrollToFile(Math.max(activeIndex.value - 1, 0));
  else if (event.key === 'n') stepThread(1);
  else if (event.key === 'p') stepThread(-1);
  else return;
  event.preventDefault();
}

function observeAll() {
  observer?.disconnect();
  const host = scroller.value;
  if (!host) return;
  observer = new IntersectionObserver(
    (entries) => {
      const fresh: number[] = [];
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        fresh.push(Number((entry.target as HTMLElement).dataset.index));
        observer?.unobserve(entry.target);
      }
      if (fresh.length) shown.value = new Set([...shown.value, ...fresh]);
    },
    { root: host, rootMargin: '800px 0px' },
  );
  for (let i = 0; i < props.files.length; i++) {
    const el = fileEls[i];
    if (el && !shown.value.has(i)) observer.observe(el);
  }
}

watch(
  () => props.files,
  async () => {
    shown.value = new Set();
    collapsed.value = new Set();
    expanded.value = new Set();
    activeIndex.value = 0;
    threadCursor.value = -1;
    fileEls.length = props.files.length;
    await nextTick();
    observeAll();
  },
);

onMounted(() => {
  fileEls.length = props.files.length;
  observeAll();
});

onBeforeUnmount(() => {
  observer?.disconnect();
  clearTimeout(flashTimer);
  if (frame) cancelAnimationFrame(frame);
});

defineExpose({ focusThread, root });
</script>

<template>
  <div ref="root" class="diff" tabindex="0" data-testid="diff-view" @keydown="onKey">
    <aside class="tree" data-testid="diff-tree">
      <div class="tree-filter">
        <Search :size="14" />
        <VInputText v-model="filter" class="tree-input" placeholder="Фильтр файлов" autocomplete="off" data-testid="diff-filter" />
      </div>
      <div class="tree-list">
        <template v-for="row in treeRows" :key="row.key">
          <button v-if="row.kind === 'dir'" class="tree-row dir" :style="{ paddingLeft: `${8 + row.depth * 12}px` }" @click="toggleDir(row)">
            <component :is="row.open ? FolderOpen : Folder" :size="14" />
            <span class="tree-name">{{ row.name }}</span>
            <span class="counts"><span class="add">+{{ row.additions }}</span> <span class="del">−{{ row.deletions }}</span></span>
          </button>
          <button
            v-else
            class="tree-row"
            :data-active="activeIndex === row.index"
            :style="{ paddingLeft: `${8 + row.depth * 12}px` }"
            :title="row.path"
            data-testid="diff-tree-file"
            @click="scrollToFile(row.index)"
          >
            <span class="mark" :data-status="files[row.index]!.status" :title="STATUS_TITLE[files[row.index]!.status]">{{ STATUS_MARK[files[row.index]!.status] }}</span>
            <span class="tree-name">{{ row.name }}</span>
            <span class="counts"><span class="add">+{{ row.additions }}</span> <span class="del">−{{ row.deletions }}</span></span>
          </button>
        </template>
        <p v-if="!treeRows.length" class="tree-empty">Ничего не найдено</p>
      </div>
    </aside>

    <div class="main">
      <header class="toolbar">
        <span class="summary">{{ files.length }} файлов <span class="add">+{{ totals.additions }}</span> <span class="del">−{{ totals.deletions }}</span></span>
        <span class="spacer" />
        <span v-if="unresolved.length" class="group">
          <button class="icon" title="Предыдущее обсуждение (p)" aria-label="Предыдущее обсуждение" data-testid="diff-prev-thread" @click="stepThread(-1)"><ChevronUp :size="14" /></button>
          <span>{{ unresolved.length }} нерешённых</span>
          <button class="icon" title="Следующее обсуждение (n)" aria-label="Следующее обсуждение" data-testid="diff-next-thread" @click="stepThread(1)"><ChevronDown :size="14" /></button>
        </span>
        <span class="group" role="group" aria-label="Режим диффа">
          <button class="mode" :data-active="mode === 'unified'" title="Единый" data-testid="diff-mode-unified" @click="setMode('unified')"><Rows2 :size="14" />Единый</button>
          <button class="mode" :data-active="mode === 'split'" title="Рядом" data-testid="diff-mode-split" @click="setMode('split')"><Columns2 :size="14" />Рядом</button>
        </span>
      </header>

      <div ref="scroller" class="files" data-testid="diff-files" @scroll.passive="onScroll">
        <p v-if="!files.length" class="empty">Нет изменённых файлов</p>
        <section v-for="(file, i) in files" :key="`${file.old_path}>${file.new_path}`" :ref="(el) => (fileEls[i] = el as HTMLElement | null)" class="file" :data-index="i" data-testid="diff-file">
          <header class="file-head" @click="toggleCollapsed(i)">
            <component :is="collapsed.has(i) ? ChevronRight : ChevronDown" :size="14" class="chev" />
            <span class="mark" :data-status="file.status" :title="STATUS_TITLE[file.status]">{{ STATUS_MARK[file.status] }}</span>
            <span class="path" :title="file.new_path">
              <template v-if="file.status === 'renamed' && file.old_path !== file.new_path">{{ file.old_path }} → </template>{{ file.new_path }}
            </span>
            <span class="counts"><span class="add">+{{ file.additions }}</span> <span class="del">−{{ file.deletions }}</span></span>
          </header>

          <template v-if="!collapsed.has(i)">
            <p v-if="note(file)" class="file-note" data-testid="diff-note">{{ note(file) }}</p>
            <DiffFileBody
              v-else-if="shown.has(i)"
              :file="file"
              :discussions="fileDiscussions[i] ?? []"
              :mode="mode"
              :expanded="expanded.has(i)"
              :flash-id="flashId"
              :connector="connector"
              @expand="expandFile(i)"
            />
            <div v-else class="placeholder" :style="{ height: `${estimates[i]}px` }" />
          </template>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.diff {
  display: flex;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  outline: none;
}

.tree {
  display: flex;
  flex: none;
  flex-direction: column;
  width: 280px;
  min-height: 0;
  border-right: 1px solid var(--vr-color-border);
}

.tree-filter {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  border-bottom: 1px solid var(--vr-color-border);
}

.tree-input {
  flex: 1;
  min-width: 0;
}

.tree-list {
  flex: 1;
  min-height: 0;
  padding: var(--vr-space-1) 0;
  overflow: auto;
}

.tree-empty {
  margin: var(--vr-space-3);
  color: var(--vr-color-text-muted);
}

.tree-row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  width: 100%;
  min-height: 26px;
  padding-right: var(--vr-space-2);
  color: inherit;
  font: inherit;
  font-size: var(--vr-font-size-xs);
  text-align: left;
  background: none;
  border: none;
  cursor: pointer;
}

.tree-row:hover {
  background: var(--vr-color-nav-hover);
}

.tree-row[data-active='true'] {
  background: var(--vr-color-nav-active);
}

.tree-row.dir {
  color: var(--vr-color-text-muted);
}

.tree-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.counts {
  flex: none;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
}

.add {
  color: var(--vr-color-success);
}

.del {
  color: var(--vr-color-danger);
}

.mark {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-sm);
}

.mark[data-status='added'] {
  color: var(--vr-color-success);
}

.mark[data-status='deleted'] {
  color: var(--vr-color-danger);
}

.mark[data-status='renamed'] {
  color: var(--vr-color-conflict);
}

.main {
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
  gap: var(--vr-space-3);
  padding: var(--vr-space-1) var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
  border-bottom: 1px solid var(--vr-color-border);
}

.spacer {
  flex: 1;
}

.group {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
}

.icon,
.mode {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 2px var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  background: none;
  border: 1px solid transparent;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.icon:hover,
.mode:hover {
  color: var(--vr-color-text);
}

.mode[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}

.files {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.empty {
  margin: var(--vr-space-4);
  color: var(--vr-color-text-muted);
}

.file {
  margin: var(--vr-space-3);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.file-head {
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-3);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md) var(--vr-radius-md) 0 0;
  cursor: pointer;
  user-select: none;
}

.chev {
  flex: none;
  color: var(--vr-color-text-muted);
}

.path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.file-note {
  margin: 0;
  padding: var(--vr-space-3);
  color: var(--vr-color-text-muted);
}
</style>
