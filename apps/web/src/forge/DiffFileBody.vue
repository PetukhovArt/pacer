<script setup lang="ts">
import { computed } from 'vue';
import type { DiffFile, Discussion } from '../connectors/api';
import { escapeHtml, highlightLines } from '../content/highlight';
import { parsePatch, sideTexts, splitRows, type PatchLine, type SplitRow } from './patch';
import ThreadView from './ThreadView.vue';

const props = defineProps<{
  file: DiffFile;
  discussions: Discussion[];
  mode: 'unified' | 'split';
  expanded: boolean;
  wrap: boolean;
  flashId: string | null;
  connector?: string;
}>();
const emit = defineEmits<{ expand: [] }>();

type Model = { lines: PatchLine[]; rows: SplitRow[]; html: string[]; threads: Map<number, Discussion[]>; orphans: Discussion[] };

const ROW_LIMIT = 2500;
const EMPTY: Discussion[] = [];

function safeHighlight(code: string, path: string): string[] {
  if (!code) return [];
  try {
    return highlightLines(code, path);
  } catch {
    return code.split('\n').map(escapeHtml);
  }
}

const model = computed<Model>(() => {
  const file = props.file;
  const lines = parsePatch(file.patch);
  const { oldCode, newCode, oldIndex, newIndex } = sideTexts(lines);
  const oldHtml = file.status === 'added' ? [] : safeHighlight(oldCode, file.old_path);
  const newHtml = file.status === 'deleted' ? [] : safeHighlight(newCode, file.new_path);
  const html = lines.map((line) => {
    if (line.kind === 'del') return oldHtml[oldIndex.get(line.index) ?? -1] ?? escapeHtml(line.text);
    if (line.kind === 'add' || line.kind === 'ctx') return newHtml[newIndex.get(line.index) ?? -1] ?? escapeHtml(line.text);
    return escapeHtml(line.text);
  });
  const byNew = new Map<number, number>();
  const byOld = new Map<number, number>();
  for (const line of lines) {
    if (line.newLine != null) byNew.set(line.newLine, line.index);
    if (line.oldLine != null) byOld.set(line.oldLine, line.index);
  }
  const threads = new Map<number, Discussion[]>();
  const orphans: Discussion[] = [];
  for (const d of props.discussions) {
    const p = d.position;
    const at = (p?.new_line != null ? byNew.get(p.new_line) : undefined) ?? (p?.old_line != null ? byOld.get(p.old_line) : undefined);
    if (at === undefined) {
      orphans.push(d);
      continue;
    }
    const list = threads.get(at);
    if (list) list.push(d);
    else threads.set(at, [d]);
  }
  return { lines, rows: splitRows(lines), html, threads, orphans };
});

const shownLines = computed(() => (props.expanded ? model.value.lines : model.value.lines.slice(0, ROW_LIMIT)));
const shownRows = computed(() => (props.expanded ? model.value.rows : model.value.rows.slice(0, ROW_LIMIT)));
const truncated = computed(() => !props.expanded && (props.mode === 'unified' ? model.value.lines.length : model.value.rows.length) > ROW_LIMIT);

function rowThreads(row: SplitRow): Discussion[] {
  const threads = model.value.threads;
  if (!threads.size) return EMPTY;
  const left = row.left ? (threads.get(row.left.index) ?? EMPTY) : EMPTY;
  if (row.right === row.left) return left;
  const right = row.right ? (threads.get(row.right.index) ?? EMPTY) : EMPTY;
  return left.length && right.length ? [...left, ...right] : left.length ? left : right;
}

const codeOf = (line: PatchLine | null) => (line ? model.value.html[line.index] || ' ' : ' ');
</script>

<template>
  <div class="body" :data-mode="mode" :data-wrap="wrap">
    <div v-for="d in model.orphans" :key="d.id" class="thread-row" :data-flash="flashId === d.id">
      <ThreadView :discussion="d" :connector="connector" show-position />
    </div>

    <div v-if="mode === 'unified'" class="lines">
      <template v-for="line in shownLines" :key="line.index">
        <div v-if="line.kind === 'hunk'" class="hunk">{{ line.text }}</div>
        <div v-else-if="line.kind === 'meta'" class="hunk meta">{{ line.text }}</div>
        <div v-else class="line" :data-kind="line.kind">
          <span class="num">{{ line.oldLine ?? '' }}</span>
          <span class="num">{{ line.newLine ?? '' }}</span>
          <span class="sign">{{ line.kind === 'add' ? '+' : line.kind === 'del' ? '−' : '' }}</span>
          <span class="code" v-html="codeOf(line)" />
        </div>
        <div v-for="d in model.threads.get(line.index)" :key="d.id" class="thread-row" :data-flash="flashId === d.id">
          <ThreadView :discussion="d" :connector="connector" />
        </div>
      </template>
    </div>

    <div v-else class="lines split">
      <template v-for="(row, r) in shownRows" :key="r">
        <div v-if="row.full" class="hunk" :class="{ meta: row.full.kind === 'meta' }">{{ row.full.text }}</div>
        <div v-else class="line split-line">
          <span class="num" :data-kind="row.left?.kind">{{ row.left?.oldLine ?? '' }}</span>
          <span class="code" :data-kind="row.left?.kind" v-html="codeOf(row.left)" />
          <span class="num" :data-kind="row.right?.kind">{{ row.right?.newLine ?? '' }}</span>
          <span class="code" :data-kind="row.right?.kind" v-html="codeOf(row.right)" />
        </div>
        <div v-for="d in rowThreads(row)" :key="d.id" class="thread-row" :data-flash="flashId === d.id">
          <ThreadView :discussion="d" :connector="connector" />
        </div>
      </template>
    </div>

    <button v-if="truncated" class="more" data-testid="diff-show-all" @click="emit('expand')">Показать весь файл</button>
  </div>
</template>

<style scoped>
.body {
  container-type: inline-size;
  overflow-x: auto;
}

.lines {
  min-width: max-content;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  line-height: 20px;
}

.lines.split {
  display: grid;
  grid-template-columns: 52px minmax(calc(50cqw - 52px), max-content) 52px minmax(calc(50cqw - 52px), max-content);
  width: max-content;
  min-width: 0;
}

.lines.split > .hunk,
.lines.split > .thread-row {
  grid-column: 1 / -1;
}

.hunk {
  padding: 0 var(--vr-space-3);
  color: var(--vr-color-text-muted);
  white-space: pre;
  background: var(--vr-color-chip-bg);
}

.hunk.meta {
  font-style: italic;
  background: none;
}

.line {
  display: grid;
  grid-template-columns: 52px 52px 16px 1fr;
}

.line[data-kind='add'] {
  background: color-mix(in srgb, var(--vr-color-success) 14%, transparent);
}

.line[data-kind='del'] {
  background: color-mix(in srgb, var(--vr-color-danger) 14%, transparent);
}

.num {
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-text-muted);
  text-align: right;
  user-select: none;
}

.sign {
  color: var(--vr-color-text-muted);
  text-align: center;
  user-select: none;
}

.code {
  white-space: pre;
}

.split-line {
  grid-column: 1 / -1;
  grid-template-columns: subgrid;
}

.split-line [data-kind='add'] {
  background: color-mix(in srgb, var(--vr-color-success) 14%, transparent);
}

.split-line [data-kind='del'] {
  background: color-mix(in srgb, var(--vr-color-danger) 14%, transparent);
}

.split-line .code + .num {
  border-left: 1px solid var(--vr-color-border);
}

.body[data-wrap='true'] .lines {
  min-width: 0;
}

.body[data-wrap='true'] .lines.split {
  grid-template-columns: 52px minmax(0, 1fr) 52px minmax(0, 1fr);
  width: auto;
}

.body[data-wrap='true'] .lines:not(.split) .line {
  grid-template-columns: 52px 52px 16px minmax(0, 1fr);
}

.body[data-wrap='true'] .code,
.body[data-wrap='true'] .hunk {
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.thread-row {
  position: sticky;
  left: 0;
  box-sizing: border-box;
  width: 100cqw;
  padding: var(--vr-space-2) var(--vr-space-3);
  font-family: var(--vr-font-family);
  white-space: normal;
}

.thread-row[data-flash='true'] {
  background: var(--vr-color-nav-active);
}

.more {
  display: block;
  width: 100%;
  padding: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  background: none;
  border: none;
  border-top: 1px solid var(--vr-color-border);
  cursor: pointer;
}

.more:hover {
  color: var(--vr-color-text);
}
</style>
