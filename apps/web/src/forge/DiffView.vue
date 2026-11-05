<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch, type ShallowRef } from 'vue';
import { ChevronDown, ChevronRight, ChevronUp, Columns2, Folder, FolderOpen, Rows2, Search, WrapText } from 'lucide-vue-next';
import { VInputText } from '@ui/kit';
import type { DiffFile, Discussion } from '../connectors/api';
import { t } from '../i18n';
import DiffFileBody from './DiffFileBody.vue';
import { isMarkdownPath } from './fileBase';
import MarkdownPreview from './MarkdownPreview.vue';

const props = defineProps<{ files: DiffFile[]; discussions: Discussion[]; connector?: string; project?: string; headSha?: string; requestUrl?: string }>();

type Mode = 'unified' | 'split';
type IndexSet = ShallowRef<ReadonlySet<number>>;
type TreeRow = { key: string; kind: 'dir' | 'file'; depth: number; name: string; path: string; index: number; additions: number; deletions: number; open: boolean };
type TreeNode = { name: string; path: string; dirs: Map<string, TreeNode>; files: number[] };
type Located = { discussion: Discussion; file: number; line: number };

const MODE_KEY = 'veyra.diff.mode';
const WRAP_KEY = 'veyra.diff.wrap';
const LINE_HEIGHT = 20;
const SETTLE_FRAMES = 8;
const SETTLE_LIMIT = 90;
const EMPTY: Discussion[] = [];
const SCROLL_KEYS: ReadonlySet<string> = new Set(['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' ']);

function storedMode(): Mode {
  try {
    return localStorage.getItem(MODE_KEY) === 'split' ? 'split' : 'unified';
  } catch {
    return 'unified';
  }
}

function storedWrap(): boolean {
  try {
    return localStorage.getItem(WRAP_KEY) === '1';
  } catch {
    return false;
  }
}

const mode = shallowRef<Mode>(storedMode());
const wrap = shallowRef(storedWrap());
const filter = shallowRef('');
const pinned = shallowRef(-1);
const collapsed: IndexSet = shallowRef(new Set<number>());
const shown: IndexSet = shallowRef(new Set<number>());
const expanded: IndexSet = shallowRef(new Set<number>());
const previewing: IndexSet = shallowRef(new Set<number>());
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
let settleFrame = 0;
let settling = false;

function setMode(next: Mode) {
  mode.value = next;
  try {
    localStorage.setItem(MODE_KEY, next);
  } catch {
    return;
  }
}

function toggleWrap() {
  wrap.value = !wrap.value;
  try {
    localStorage.setItem(WRAP_KEY, wrap.value ? '1' : '0');
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

function canPreview(file: DiffFile): boolean {
  return Boolean(props.connector && props.project && props.headSha && props.requestUrl) && file.status !== 'deleted' && isMarkdownPath(file.new_path);
}

function setPreview(index: number, on: boolean) {
  if (on) include(previewing, index);
  else exclude(previewing, index);
}

const totals = computed(() => props.files.reduce((acc, f) => ({ additions: acc.additions + f.additions, deletions: acc.deletions + f.deletions }), { additions: 0, deletions: 0 }));

const query = computed(() => filter.value.trim().toLowerCase());

const visible = computed<number[]>(() => {
  const needle = query.value;
  const indexes: number[] = [];
  props.files.forEach((file, index) => {
    if (!needle || file.new_path.toLowerCase().includes(needle) || file.old_path.toLowerCase().includes(needle)) indexes.push(index);
  });
  return indexes;
});

const visibleSet = computed(() => new Set(visible.value));
const entries = computed(() => visible.value.map((i) => ({ file: props.files[i]!, i })));

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
  if (file.binary) return t('forge.diff.note.binary');
  if (file.too_large) return t('forge.diff.note.tooLarge');
  if (!file.patch) return file.status === 'renamed' ? t('forge.diff.note.renamed') : t('forge.diff.note.unchanged');
  return null;
}

const STATUS_MARK: Record<DiffFile['status'], string> = { added: 'A', deleted: 'D', modified: 'M', renamed: 'R' };
const STATUS_TITLE_KEY = { added: 'forge.diff.status.added', deleted: 'forge.diff.status.deleted', modified: 'forge.diff.status.modified', renamed: 'forge.diff.status.renamed' } as const;

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
  if (query.value) {
    return visible.value.map((index) => {
      const file = props.files[index]!;
      return { key: `f${index}`, kind: 'file' as const, depth: 0, name: file.new_path, path: file.new_path, index, additions: file.additions, deletions: file.deletions, open: false };
    });
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

function release() {
  pinned.value = -1;
  settling = false;
  cancelAnimationFrame(settleFrame);
}

function settle(index: number) {
  cancelAnimationFrame(settleFrame);
  settling = true;
  let stable = 0;
  let frames = 0;
  let last = Number.NaN;
  const step = () => {
    if (pinned.value !== index) return;
    alignTo(index);
    const top = fileEls[index]?.offsetTop ?? Number.NaN;
    stable = top === last ? stable + 1 : 0;
    last = top;
    if (stable >= SETTLE_FRAMES || ++frames >= SETTLE_LIMIT) {
      settling = false;
      return;
    }
    settleFrame = requestAnimationFrame(step);
  };
  step();
}

async function scrollToFile(index: number) {
  if (index < 0 || index >= props.files.length) return;
  if (!visibleSet.value.has(index)) {
    filter.value = '';
    await nextTick();
  }
  activeIndex.value = index;
  pinned.value = index;
  exclude(collapsed, index);
  include(shown, index);
  alignTo(index);
  await nextTick();
  settle(index);
}

function trackActive() {
  frame = 0;
  const host = scroller.value;
  if (!host) return;
  if (pinned.value >= 0) {
    const el = fileEls[pinned.value];
    const atEnd = host.scrollTop + host.clientHeight >= host.scrollHeight - 2;
    if (settling || (el && (Math.abs(el.offsetTop - host.scrollTop) <= 2 || atEnd))) {
      activeIndex.value = pinned.value;
      return;
    }
    pinned.value = -1;
  }
  const edge = host.scrollTop + 8;
  let current = visible.value[0] ?? 0;
  for (const i of visible.value) {
    const el = fileEls[i];
    if (!el) continue;
    if (el.offsetTop > edge) break;
    current = i;
  }
  activeIndex.value = current;
}

function stepFile(delta: 1 | -1) {
  const list = visible.value;
  if (!list.length) return;
  const at = list.indexOf(activeIndex.value);
  let next: number;
  if (at >= 0) next = list[Math.min(Math.max(at + delta, 0), list.length - 1)]!;
  else next = delta > 0 ? (list.find((i) => i > activeIndex.value) ?? list.at(-1)!) : ([...list].reverse().find((i) => i < activeIndex.value) ?? list[0]!);
  void scrollToFile(next);
}

function onScroll() {
  if (!frame) frame = requestAnimationFrame(trackActive);
}

const unresolved = computed<Located[]>(() => {
  const located: Located[] = [];
  fileDiscussions.value.forEach((list, file) => {
    if (!visibleSet.value.has(file)) return;
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
  release();
  if (!visibleSet.value.has(fileIndex)) {
    filter.value = '';
    await nextTick();
  }
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
  if (event.key === 'j') stepFile(1);
  else if (event.key === 'k') stepFile(-1);
  else if (event.key === 'n') stepThread(1);
  else if (event.key === 'p') stepThread(-1);
  else {
    if (SCROLL_KEYS.has(event.key)) release();
    return;
  }
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

watch(visible, async (list) => {
  release();
  threadCursor.value = -1;
  if (!list.includes(activeIndex.value)) activeIndex.value = list[0] ?? 0;
  scroller.value?.scrollTo({ top: 0 });
  await nextTick();
  observeAll();
});

watch(
  () => props.files,
  async () => {
    release();
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
  cancelAnimationFrame(settleFrame);
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
        <VInputText v-model="filter" class="tree-input" :placeholder="t('forge.diff.filter')" autocomplete="off" data-testid="diff-filter" />
      </div>
      <div v-auto-animate class="tree-list">
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
            <span class="mark" :data-status="files[row.index]!.status" :title="t(STATUS_TITLE_KEY[files[row.index]!.status])">{{ STATUS_MARK[files[row.index]!.status] }}</span>
            <span class="tree-name">{{ row.name }}</span>
            <span class="counts"><span class="add">+{{ row.additions }}</span> <span class="del">−{{ row.deletions }}</span></span>
          </button>
        </template>
        <p v-if="!treeRows.length" class="tree-empty">{{ t('common.noResults') }}</p>
      </div>
    </aside>

    <div class="main">
      <header class="toolbar">
        <span class="summary">{{ t('forge.diff.fileCount', { count: files.length }) }} <span class="add">+{{ totals.additions }}</span> <span class="del">−{{ totals.deletions }}</span></span>
        <span class="spacer" />
        <span v-if="unresolved.length" class="group">
          <button class="icon" :title="t('forge.diff.prevThreadTitle')" :aria-label="t('forge.diff.prevThread')" data-testid="diff-prev-thread" @click="stepThread(-1)"><ChevronUp :size="14" /></button>
          <span>{{ t('forge.diff.unresolvedCount', { count: unresolved.length }) }}</span>
          <button class="icon" :title="t('forge.diff.nextThreadTitle')" :aria-label="t('forge.diff.nextThread')" data-testid="diff-next-thread" @click="stepThread(1)"><ChevronDown :size="14" /></button>
        </span>
        <button class="mode" :data-active="wrap" :aria-pressed="wrap" :title="t('forge.diff.wrap')" data-testid="diff-wrap" @click="toggleWrap"><WrapText :size="14" />{{ t('forge.diff.wrap') }}</button>
        <span class="group" role="group" :aria-label="t('forge.diff.modeLabel')">
          <button class="mode" :data-active="mode === 'unified'" :title="t('forge.diff.unified')" data-testid="diff-mode-unified" @click="setMode('unified')"><Rows2 :size="14" />{{ t('forge.diff.unified') }}</button>
          <button class="mode" :data-active="mode === 'split'" :title="t('forge.diff.split')" data-testid="diff-mode-split" @click="setMode('split')"><Columns2 :size="14" />{{ t('forge.diff.split') }}</button>
        </span>
      </header>

      <div ref="scroller" v-auto-animate class="files" data-testid="diff-files" @scroll.passive="onScroll" @wheel.passive="release" @pointerdown="release" @touchstart.passive="release">
        <p v-if="!files.length" class="empty">{{ t('forge.diff.noFiles') }}</p>
        <p v-else-if="!visible.length" class="empty">{{ t('common.noResults') }}</p>
        <section v-for="{ file, i } in entries" :key="`${file.old_path}>${file.new_path}`" :ref="(el) => (fileEls[i] = el as HTMLElement | null)" class="file" :data-index="i" data-testid="diff-file">
          <header class="file-head" @click="toggleCollapsed(i)">
            <ChevronRight :size="14" class="chev vr-chevron" :data-open="!collapsed.has(i)" />
            <span class="mark" :data-status="file.status" :title="t(STATUS_TITLE_KEY[file.status])">{{ STATUS_MARK[file.status] }}</span>
            <span class="path" :title="file.new_path">
              <template v-if="file.status === 'renamed' && file.old_path !== file.new_path">{{ file.old_path }} → </template>{{ file.new_path }}
            </span>
            <span v-if="canPreview(file)" class="group" role="group" :aria-label="t('forge.diff.fileView')" @click.stop>
              <button class="mode" :data-active="!previewing.has(i)" data-testid="diff-file-code" @click="setPreview(i, false)">{{ t('forge.diff.code') }}</button>
              <button class="mode" :data-active="previewing.has(i)" data-testid="diff-file-preview" @click="setPreview(i, true)">{{ t('forge.diff.preview') }}</button>
            </span>
            <span class="counts"><span class="add">+{{ file.additions }}</span> <span class="del">−{{ file.deletions }}</span></span>
          </header>

          <template v-if="!collapsed.has(i)">
            <MarkdownPreview
              v-if="previewing.has(i) && canPreview(file)"
              :connector="connector!"
              :project="project!"
              :sha="headSha!"
              :path="file.new_path"
              :request-url="requestUrl!"
            />
            <p v-else-if="note(file)" class="file-note" data-testid="diff-note">{{ note(file) }}</p>
            <DiffFileBody
              v-else-if="shown.has(i)"
              :file="file"
              :discussions="fileDiscussions[i] ?? []"
              :mode="mode"
              :expanded="expanded.has(i)"
              :wrap="wrap"
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
