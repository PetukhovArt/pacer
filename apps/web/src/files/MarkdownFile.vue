<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { ChevronDown, Columns2, ExternalLink, FileCode, ListTree, Eye, Save } from 'lucide-vue-next';
import { EditorState, type Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { describeError } from '../connectors/api';
import RichContent from '../content/RichContent.vue';
import { useDarkTheme } from '../content/theme';
import { t, useI18n } from '../i18n';
import { canOpenExternal, fsOpenExternal } from './api';
import CompareView from './CompareView.vue';
import { createExtensions, darkSlot, languageSlot, phrases, scrollTopOfLine, topLineOf } from './editor';
import { useEditableDoc } from './useEditableDoc';

type Mode = 'code' | 'split' | 'preview';
type Heading = { id: string; level: number; text: string; line: number };
type Anchor = { line: number; top: number };

const props = defineProps<{ path: string; hash?: string }>();
const emit = defineEmits<{ 'open-file': [path: string, hash: string] }>();

const MODE_KEY = 'berth.markdown.mode';
const END_SLACK = 2;

function storedMode(): Mode {
  try {
    const value = localStorage.getItem(MODE_KEY);
    return value === 'code' || value === 'preview' ? value : 'split';
  } catch {
    return 'split';
  }
}

const mode = shallowRef<Mode>(storedMode());
const externalFailure = shallowRef<string | null>(null);
const headings = shallowRef<Heading[]>([]);
const tocOpen = shallowRef(false);
const comparing = shallowRef(false);
const mineSnapshot = shallowRef('');

const sourcePane = useTemplateRef<HTMLElement>('source');
const previewPane = useTemplateRef<HTMLElement>('preview');
const toc = useTemplateRef<HTMLElement>('toc');

const { locale } = useI18n();
const dark = useDarkTheme();

let view: EditorView | undefined;
let extensions: Extension[] = [];

const doc = useEditableDoc({
  path: () => props.path,
  view: () => view,
  reset: (text) => view?.setState(EditorState.create({ doc: text, extensions })),
  onReplaced: () => {
    anchorsDirty = true;
  },
});
const { previewText, failure: loadFailure, saveError, dirty, saving, loaded, conflict } = doc;

const failure = computed(() => loadFailure.value ?? externalFailure.value);
const fileName = computed(() => props.path.split(/[\\/]/).pop() ?? props.path);
const directory = computed(() => props.path.replace(/[\\/][^\\/]*$/, '') || props.path);
const isDiagram = computed(() => /\.(mmd|mermaid)$/i.test(props.path));
const renderedText = computed(() => (previewText.value !== null && isDiagram.value ? `\`\`\`mermaid\n${previewText.value}\n\`\`\`` : previewText.value));
const showSource = computed(() => mode.value !== 'preview');
const showPreview = computed(() => mode.value !== 'code');

let anchors: Anchor[] = [];
let anchorsDirty = true;
let topLine = 0;
let ignore: { el: HTMLElement; top: number } | null = null;
let pendingHash = props.hash ?? '';
let resizer: ResizeObserver | undefined;

function setMode(next: Mode) {
  mode.value = next;
  try {
    localStorage.setItem(MODE_KEY, next);
  } catch {
    return;
  }
}

function setTop(el: HTMLElement, top: number) {
  const max = Math.max(0, el.scrollHeight - el.clientHeight);
  el.scrollTop = Math.min(max, Math.max(0, top));
  ignore = { el, top: el.scrollTop };
}

function collectAnchors(pane: HTMLElement): Anchor[] {
  const base = pane.getBoundingClientRect().top - pane.scrollTop;
  const found: Anchor[] = [];
  for (const el of pane.querySelectorAll<HTMLElement>('[data-line]')) {
    const line = Number(el.dataset.line);
    if (!Number.isFinite(line)) continue;
    const top = el.getBoundingClientRect().top - base;
    const last = found[found.length - 1];
    if (last && (line < last.line || top < last.top)) continue;
    found.push({ line, top });
  }
  return found;
}

function currentAnchors(pane: HTMLElement): Anchor[] {
  if (anchorsDirty) {
    anchors = collectAnchors(pane);
    anchorsDirty = false;
  }
  return anchors;
}

function lineCount(): number {
  return Math.max(1, view?.state.doc.lines ?? 1);
}

function lineToTop(pane: HTMLElement, line: number): number {
  const list = currentAnchors(pane);
  const first = list[0];
  if (!first) return 0;
  if (line <= first.line) return first.top * (line / Math.max(first.line, 1));
  let index = 0;
  for (let i = 0; i < list.length; i++) {
    if ((list[i] as Anchor).line <= line) index = i;
    else break;
  }
  const from = list[index] as Anchor;
  const to = list[index + 1] ?? { line: lineCount(), top: pane.scrollHeight - pane.clientHeight + pane.clientHeight * 0.5 };
  const span = to.line - from.line;
  return from.top + (span > 0 ? ((to.top - from.top) * (line - from.line)) / span : 0);
}

function topToLine(pane: HTMLElement, top: number): number {
  const list = currentAnchors(pane);
  const first = list[0];
  if (!first) return 0;
  if (top <= first.top) return first.top > 0 ? (first.line * top) / first.top : first.line;
  let index = 0;
  for (let i = 0; i < list.length; i++) {
    if ((list[i] as Anchor).top <= top) index = i;
    else break;
  }
  const from = list[index] as Anchor;
  const to = list[index + 1] ?? { line: lineCount(), top: pane.scrollHeight };
  const span = to.top - from.top;
  return from.line + (span > 0 ? ((to.line - from.line) * (top - from.top)) / span : 0);
}

function atEnd(el: HTMLElement): boolean {
  return el.scrollHeight > el.clientHeight && el.scrollTop >= el.scrollHeight - el.clientHeight - END_SLACK;
}

function consumed(el: HTMLElement): boolean {
  if (!ignore || ignore.el !== el) return false;
  const same = Math.abs(el.scrollTop - ignore.top) < 1.5;
  ignore = null;
  return same;
}

function onSourceScroll() {
  const source = view?.scrollDOM;
  if (!view || !source || consumed(source)) return;
  topLine = topLineOf(view);
  const preview = previewPane.value;
  if (mode.value !== 'split' || !preview) return;
  const target = atEnd(source) ? preview.scrollHeight : lineToTop(preview, topLine);
  setTop(preview, target);
}

function onPreviewScroll() {
  const preview = previewPane.value;
  if (!preview || consumed(preview)) return;
  topLine = topToLine(preview, preview.scrollTop);
  const source = view?.scrollDOM;
  if (!view || mode.value !== 'split' || !source) return;
  const target = atEnd(preview) ? source.scrollHeight : scrollTopOfLine(view, topLine);
  setTop(source, target);
}

function applyTopLine(includeSource = true) {
  const preview = previewPane.value;
  if (view && includeSource && showSource.value) setTop(view.scrollDOM, scrollTopOfLine(view, topLine));
  if (preview && showPreview.value) setTop(preview, lineToTop(preview, topLine));
}

function collectHeadings() {
  const pane = previewPane.value;
  if (!pane) return;
  headings.value = [...pane.querySelectorAll<HTMLElement>('h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]')]
    .filter((el) => !el.closest('.footnotes'))
    .map((el) => ({ id: el.id, level: Number(el.tagName.slice(1)), text: el.textContent.trim(), line: Number(el.dataset.line) || 0 }));
}

function onRendered() {
  anchorsDirty = true;
  collectHeadings();
  void nextTick(() => {
    if (mode.value === 'split') applyTopLine(false);
    applyHash();
  });
}

function gotoHeading(heading: Heading) {
  tocOpen.value = false;
  if (mode.value === 'code' && view) {
    topLine = heading.line;
    setTop(view.scrollDOM, scrollTopOfLine(view, heading.line));
    return;
  }
  scrollPreviewTo(heading.id);
}

function scrollPreviewTo(id: string): boolean {
  const preview = previewPane.value;
  const target = preview?.querySelector(`[id="${CSS.escape(id)}"]`);
  if (!preview || !target) return false;
  const top = target.getBoundingClientRect().top - preview.getBoundingClientRect().top + preview.scrollTop;
  preview.scrollTop = Math.max(0, top - 8);
  return true;
}

function applyHash() {
  if (!pendingHash || mode.value === 'code') return;
  if (scrollPreviewTo(pendingHash)) pendingHash = '';
}

function onPointerDown(event: PointerEvent) {
  if (tocOpen.value && event.target instanceof Node && !toc.value?.contains(event.target)) tocOpen.value = false;
}

function onKeydown(event: KeyboardEvent) {
  if (!(event.ctrlKey || event.metaKey) || event.shiftKey || event.altKey || event.code !== 'KeyS') return;
  event.preventDefault();
  void doc.save();
}

async function openExternal() {
  try {
    await fsOpenExternal(props.path);
  } catch (e) {
    externalFailure.value = describeError(e);
  }
}

function openFile() {
  topLine = 0;
  externalFailure.value = null;
  comparing.value = false;
  void doc.load();
}

function toggleCompare() {
  mineSnapshot.value = doc.mine();
  comparing.value = !comparing.value;
}

function onReload() {
  comparing.value = false;
  doc.reload();
}

function onKeepMine() {
  comparing.value = false;
  void doc.keepMine();
}

watch(() => props.path, openFile);
watch(
  () => props.hash,
  (hash) => {
    pendingHash = hash ?? '';
    applyHash();
  },
);
watch(mode, async () => {
  anchorsDirty = true;
  await nextTick();
  view?.requestMeasure();
  applyTopLine();
});
watch(conflict, (value) => {
  if (!value) comparing.value = false;
});
watch(locale, (value) => view?.dispatch({ effects: languageSlot.reconfigure(phrases(value)) }));
watch(dark, (value) => view?.dispatch({ effects: darkSlot.reconfigure(EditorView.darkTheme.of(value)) }));

onMounted(() => {
  const parent = sourcePane.value;
  if (parent) {
    extensions = createExtensions({ onUpdate: doc.onUpdate, onBlur: doc.blurred }, locale.value, dark.value);
    view = new EditorView({ state: EditorState.create({ doc: '', extensions }), parent });
    view.scrollDOM.addEventListener('scroll', onSourceScroll, { passive: true });
  }
  openFile();
  document.addEventListener('pointerdown', onPointerDown, true);
  resizer = new ResizeObserver(() => {
    anchorsDirty = true;
  });
  const content = previewPane.value?.firstElementChild;
  if (content) resizer.observe(content);
});

onBeforeUnmount(() => {
  doc.dispose();
  view?.scrollDOM.removeEventListener('scroll', onSourceScroll);
  view?.destroy();
  view = undefined;
  resizer?.disconnect();
  document.removeEventListener('pointerdown', onPointerDown, true);
});
</script>

<template>
  <section class="md" data-testid="markdown-file" @keydown="onKeydown">
    <header class="toolbar">
      <span class="name" :title="path" data-testid="markdown-name">{{ fileName }}</span>
      <span v-if="dirty" class="dirty" :title="t('files.editor.dirty')" :aria-label="t('files.editor.dirty')" data-testid="markdown-dirty" />
      <span class="group" role="group" :aria-label="t('files.markdown.viewMode')">
        <button class="mode" :data-active="mode === 'code'" :aria-pressed="mode === 'code'" data-testid="markdown-mode-code" @click="setMode('code')"><FileCode :size="14" />{{ t('files.markdown.code') }}</button>
        <button class="mode" :data-active="mode === 'split'" :aria-pressed="mode === 'split'" data-testid="markdown-mode-split" @click="setMode('split')"><Columns2 :size="14" />{{ t('files.markdown.split') }}</button>
        <button class="mode" :data-active="mode === 'preview'" :aria-pressed="mode === 'preview'" data-testid="markdown-mode-preview" @click="setMode('preview')"><Eye :size="14" />{{ t('files.markdown.preview') }}</button>
      </span>
      <button class="mode" :disabled="!dirty || saving" :title="t('files.editor.saveTitle')" data-testid="markdown-save" @click="doc.save()"><Save :size="14" />{{ t('common.save') }}</button>
      <span class="spacer" />
      <span ref="toc" class="toc-wrap">
        <button class="mode" :data-active="tocOpen" :disabled="!headings.length" aria-haspopup="menu" :aria-expanded="tocOpen" data-testid="markdown-toc" @click="tocOpen = !tocOpen">
          <ListTree :size="14" />{{ t('files.markdown.toc') }}<ChevronDown :size="12" />
        </button>
        <div v-if="tocOpen" class="toc" role="menu" data-testid="markdown-toc-list">
          <button v-for="heading in headings" :key="heading.id" class="toc-item" role="menuitem" :style="{ paddingLeft: `${8 + (heading.level - 1) * 12}px` }" :title="heading.text" @click="gotoHeading(heading)">
            {{ heading.text }}
          </button>
        </div>
      </span>
      <button v-if="canOpenExternal" class="mode" :title="t('files.markdown.openExternalTitle')" data-testid="markdown-open-external" @click="openExternal"><ExternalLink :size="14" />{{ t('files.markdown.openExternal') }}</button>
    </header>

    <div v-if="conflict" class="bar" role="alert" data-testid="markdown-conflict">
      <strong>{{ t('files.editor.changed.title') }}</strong>
      <span class="hint">{{ t('files.editor.changed.hint') }}</span>
      <span class="spacer" />
      <button class="mode" data-testid="markdown-conflict-reload" @click="onReload">{{ t('files.editor.changed.reload') }}</button>
      <button class="mode" data-testid="markdown-conflict-keep" @click="onKeepMine">{{ t('files.editor.changed.keep') }}</button>
      <button class="mode" :data-active="comparing" :aria-pressed="comparing" data-testid="markdown-conflict-compare" @click="toggleCompare">{{ t('files.editor.changed.compare') }}</button>
    </div>

    <p v-if="failure" class="status failure" role="alert" data-testid="markdown-failure">{{ failure }}</p>
    <p v-else-if="!loaded" class="status">{{ t('common.loading') }}</p>
    <p v-if="saveError" class="status failure" role="alert" data-testid="markdown-save-error">{{ saveError }}</p>

    <CompareView v-if="comparing && conflict" class="compare" :disk="conflict.text" :mine="mineSnapshot" />

    <div v-show="loaded && !comparing" class="panes" :data-mode="mode">
      <div v-show="showSource" ref="source" class="pane source" data-testid="markdown-source" />
      <div v-show="showPreview" ref="preview" class="pane preview" tabindex="0" data-testid="markdown-preview" @scroll.passive="onPreviewScroll">
        <RichContent v-if="renderedText !== null" :markdown="renderedText" :base-path="directory" variant="document" :breaks="false" @open-file="(p, h) => emit('open-file', p, h)" @rendered="onRendered" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.md {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
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

.name {
  min-width: 0;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dirty {
  flex: none;
  width: 8px;
  height: 8px;
  margin-left: calc(-1 * var(--vr-space-1));
  background: var(--vr-color-accent);
  border-radius: var(--vr-radius-full);
}

.bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-1) var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-surface-raised);
  border-bottom: 1px solid var(--vr-color-border);
}

.bar .mode {
  color: var(--vr-color-text);
  border-color: var(--vr-color-border);
}

.hint {
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.compare {
  flex: 1;
}

.spacer {
  flex: 1;
}

.group {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
}

.mode {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 2px var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  white-space: nowrap;
  background: none;
  border: 1px solid transparent;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.mode:hover:not(:disabled) {
  color: var(--vr-color-text);
}

.mode:disabled {
  opacity: 0.5;
  cursor: default;
}

.mode[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}

.toc-wrap {
  position: relative;
}

.toc {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  z-index: 10;
  display: flex;
  flex-direction: column;
  width: 320px;
  max-height: 60vh;
  padding: var(--vr-space-1) 0;
  overflow: auto;
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
  box-shadow: var(--vr-elevation-popover);
}

.toc-item {
  padding: 3px var(--vr-space-2);
  overflow: hidden;
  color: inherit;
  font: inherit;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  background: none;
  border: none;
  cursor: pointer;
}

.toc-item:hover {
  background: var(--vr-color-nav-hover);
}

.status {
  margin: var(--vr-space-4);
  color: var(--vr-color-text-muted);
}

.failure {
  color: var(--vr-color-danger);
}

.panes {
  display: grid;
  flex: 1;
  grid-template-columns: minmax(0, 1fr);
  min-height: 0;
}

.panes[data-mode='split'] {
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
}

.panes[data-mode='split'] .source {
  border-right: 1px solid var(--vr-color-border);
}

.pane {
  position: relative;
  min-height: 0;
  overflow: auto;
  outline: none;
}

.source {
  overflow: hidden;
}

.source :deep(.cm-editor) {
  height: 100%;
}

.preview {
  padding: var(--vr-space-6);
}
</style>
