<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { openUrl } from '../connectors/api';
import { fsOpenExternal } from '../files/api';
import { isRelative, loadLocalImages, loadRemoteImages, localTarget, resolveAgainst } from './assets';
import { emojiLoaded, hasShortcode, loadEmoji } from './emoji';
import { renderMath } from './math';
import MermaidView from './MermaidView.vue';
import { renderRich } from './render';

type Diagram = { key: string; el: HTMLElement; source: string };

const MARKDOWN_LINK = /\.(?:md|markdown|mdx)$/i;
const COPY_FEEDBACK_MS = 1500;

const props = withDefaults(
  defineProps<{
    markdown?: string;
    html?: string;
    connector?: string;
    baseUrl?: string;
    basePath?: string;
    variant?: 'compact' | 'document';
    breaks?: boolean;
  }>(),
  { variant: 'compact', breaks: true },
);
const emit = defineEmits<{ 'open-file': [path: string, hash: string]; rendered: [] }>();

const body = useTemplateRef<HTMLElement>('body');
const diagrams = shallowRef<Diagram[]>([]);
const emojiTick = shallowRef(0);
const rendered = computed(() => {
  void emojiTick.value;
  return renderRich({ markdown: props.markdown, html: props.html, breaks: props.breaks });
});

let generation = 0;
let copyTimer: ReturnType<typeof setTimeout> | undefined;

function sync() {
  const el = body.value;
  if (!el) return;
  const current = ++generation;
  const isCurrent = () => current === generation;
  const sources = rendered.value.diagrams;
  diagrams.value = [...el.querySelectorAll<HTMLElement>('[data-rc-mermaid]')].map((target, index) => ({
    key: `${current}-${index}`,
    el: target,
    source: sources[Number(target.dataset.rcMermaid)] ?? '',
  }));
  if (props.basePath) void loadLocalImages(el, props.basePath, isCurrent);
  else if (props.connector || props.baseUrl) void loadRemoteImages(el, props.connector, props.baseUrl, isCurrent);
  void renderMath(el, isCurrent);
  emit('rendered');
}

function jumpTo(id: string) {
  const el = body.value;
  if (!el || !id) return;
  const target = el.querySelector(`[id="${CSS.escape(id)}"], a[name="${CSS.escape(id)}"]`);
  target?.scrollIntoView({ block: 'start' });
}

async function copyCode(button: Element) {
  const code = button.closest('.rc-codeblock')?.querySelector('pre')?.textContent ?? '';
  const state = button instanceof HTMLElement ? button : null;
  try {
    await navigator.clipboard.writeText(code.replace(/\n$/, ''));
    if (state) state.dataset.state = 'ok';
  } catch {
    if (state) state.dataset.state = 'fail';
  }
  clearTimeout(copyTimer);
  copyTimer = setTimeout(() => {
    if (state) delete state.dataset.state;
  }, COPY_FEEDBACK_MS);
}

async function openLocal(raw: string): Promise<boolean> {
  if (!props.basePath || !isRelative(raw)) return false;
  const target = localTarget(props.basePath, raw);
  if (!target) return true;
  if (MARKDOWN_LINK.test(target.path)) emit('open-file', target.path, target.hash);
  else await fsOpenExternal(target.path).catch(() => undefined);
  return true;
}

async function onClick(event: MouseEvent) {
  if (!(event.target instanceof Element)) return;
  const copy = event.target.closest('.rc-copy');
  if (copy && body.value?.contains(copy)) {
    event.preventDefault();
    await copyCode(copy);
    return;
  }
  const anchor = event.target.closest('a[href]');
  if (!anchor || !body.value?.contains(anchor)) return;
  const raw = anchor.getAttribute('href') ?? '';
  event.preventDefault();
  if (raw.startsWith('#')) {
    jumpTo(decodeURIComponent(raw.slice(1)));
    return;
  }
  if (await openLocal(raw)) return;
  const url = await resolveAgainst(props.connector, raw, props.baseUrl);
  if (url && /^(?:https?|mailto):/i.test(url)) await openUrl(url);
}

watch(
  () => props.markdown,
  (text) => {
    if (text && !emojiLoaded() && hasShortcode(text)) void loadEmoji().then(() => emojiTick.value++);
  },
  { immediate: true },
);
watch([rendered, () => props.connector, () => props.baseUrl, () => props.basePath], sync, { flush: 'post' });
onMounted(sync);
onBeforeUnmount(() => clearTimeout(copyTimer));
</script>

<template>
  <div class="rich-content" :data-variant="variant" data-testid="rich-content">
    <div ref="body" class="body" @click="onClick" v-html="rendered.html" />
    <Teleport v-for="diagram in diagrams" :key="diagram.key" :to="diagram.el">
      <MermaidView :source="diagram.source" />
    </Teleport>
  </div>
</template>

<style scoped>
.rich-content {
  --rc-link: color-mix(in srgb, var(--vr-palette-blue-500) 75%, var(--vr-color-text));
  --rc-zebra: color-mix(in srgb, var(--vr-color-text) 4%, transparent);
  --rc-code-bg: color-mix(in srgb, var(--vr-color-text) 6%, transparent);
  --rc-size: var(--vr-font-size-sm);
  --rc-leading: 1.7;
  --rc-h1: 1.7em;
  --rc-h2: 1.43em;
  --rc-h3: 1.29em;
  --rc-h4: 1.14em;
  --rc-h5: 1em;
  --rc-h6: 1em;
  color: var(--vr-color-text);
  font-size: var(--rc-size);
  line-height: var(--rc-leading);
  overflow-wrap: anywhere;
}

.rich-content[data-variant='document'] {
  --rc-size: var(--vr-font-size-base);
  --rc-leading: 1.6;
  --rc-h1: 2em;
  --rc-h2: 1.5em;
  --rc-h3: 1.25em;
  --rc-h4: 1em;
  --rc-h5: 0.875em;
  --rc-h6: 0.85em;
  max-width: 980px;
}

.body > :deep(:first-child) {
  margin-top: 0;
}

.body > :deep(:last-child) {
  margin-bottom: 0;
}

.rich-content:not([data-variant='document']) .body :deep(p),
.rich-content:not([data-variant='document']) .body :deep(ul),
.rich-content:not([data-variant='document']) .body :deep(ol),
.rich-content:not([data-variant='document']) .body :deep(dl),
.rich-content:not([data-variant='document']) .body :deep(blockquote),
.rich-content:not([data-variant='document']) .body :deep(h1),
.rich-content:not([data-variant='document']) .body :deep(h2),
.rich-content:not([data-variant='document']) .body :deep(h3),
.rich-content:not([data-variant='document']) .body :deep(h4),
.rich-content:not([data-variant='document']) .body :deep(h5),
.rich-content:not([data-variant='document']) .body :deep(h6) {
  max-width: 80ch;
}

.body :deep(p),
.body :deep(ul),
.body :deep(ol),
.body :deep(dl),
.body :deep(blockquote),
.body :deep(details),
.body :deep(.rc-table),
.body :deep(.rc-codeblock),
.body :deep(.rc-diagram),
.body :deep(.rc-alert),
.body :deep(.rc-frontmatter),
.body :deep(.rc-math[data-display='block']) {
  margin: 0 0 var(--vr-space-4);
}

.body :deep(h1),
.body :deep(h2),
.body :deep(h3),
.body :deep(h4),
.body :deep(h5),
.body :deep(h6) {
  margin: var(--vr-space-6) 0 var(--vr-space-4);
  line-height: 1.25;
  font-weight: 600;
  scroll-margin-top: var(--vr-space-4);
}

.body :deep(h1) {
  padding-bottom: 0.3em;
  border-bottom: 1px solid var(--vr-color-border);
  font-size: var(--rc-h1);
}

.body :deep(h2) {
  padding-bottom: 0.3em;
  border-bottom: 1px solid var(--vr-color-border);
  font-size: var(--rc-h2);
}

.body :deep(h3) {
  font-size: var(--rc-h3);
}

.body :deep(h4) {
  font-size: var(--rc-h4);
}

.body :deep(h5) {
  font-size: var(--rc-h5);
}

.body :deep(h6) {
  color: var(--vr-color-text-muted);
  font-size: var(--rc-h6);
}

.body :deep(.rc-anchor) {
  margin-left: 0.4em;
  color: var(--vr-color-text-muted);
  font-weight: 400;
  text-decoration: none;
  opacity: 0;
}

.body :deep(.rc-anchor::before) {
  content: '#';
}

.body :deep(h1:hover .rc-anchor),
.body :deep(h2:hover .rc-anchor),
.body :deep(h3:hover .rc-anchor),
.body :deep(h4:hover .rc-anchor),
.body :deep(h5:hover .rc-anchor),
.body :deep(h6:hover .rc-anchor),
.body :deep(.rc-anchor:focus-visible) {
  opacity: 1;
}

.body :deep(.rc-anchor:hover) {
  color: var(--rc-link);
}

.body :deep(ul),
.body :deep(ol) {
  padding-left: 2em;
}

.body :deep(ul) {
  list-style: disc;
}

.body :deep(ul ul) {
  list-style: circle;
}

.body :deep(ul ul ul) {
  list-style: square;
}

.body :deep(ol) {
  list-style: decimal;
}

.body :deep(ol ol) {
  list-style: lower-alpha;
}

.body :deep(ol ol ol) {
  list-style: lower-roman;
}

.body :deep(li > ul),
.body :deep(li > ol) {
  margin: 0;
}

.body :deep(li + li) {
  margin-top: 0.25em;
}

.body :deep(li > p) {
  margin: 0 0 var(--vr-space-2);
}

.body :deep(li::marker) {
  color: var(--vr-color-text-muted);
}

.body :deep(dt) {
  margin-top: var(--vr-space-4);
  font-style: italic;
  font-weight: 600;
}

.body :deep(dd) {
  margin: 0 0 var(--vr-space-4);
  padding: 0 var(--vr-space-4);
}

.body :deep(.rc-task) {
  list-style: none;
}

.body :deep(.rc-task input) {
  margin: 0 0.35em 0.25em -1.4em;
  vertical-align: middle;
  accent-color: var(--rc-link);
}

.body :deep(blockquote) {
  padding: 0 1em;
  border-left: 0.25em solid var(--vr-color-border);
  color: var(--vr-color-text-muted);
}

.body :deep(blockquote > :last-child) {
  margin-bottom: 0;
}

.body :deep(blockquote > :first-child) {
  margin-top: 0;
}

.body :deep(a) {
  color: var(--rc-link);
  text-decoration: none;
}

.body :deep(a:hover) {
  text-decoration: underline;
  text-underline-offset: 2px;
}

.body :deep(hr) {
  height: 0.25em;
  margin: var(--vr-space-6) 0;
  padding: 0;
  border: 0;
  background: var(--vr-color-border);
}

.body :deep(img) {
  max-width: 100%;
  height: auto;
}

.body :deep(strong) {
  font-weight: 600;
}

.body :deep(:not(pre) > code) {
  padding: 0.2em 0.4em;
  border-radius: var(--vr-radius-sm);
  background: var(--rc-code-bg);
  font-family: var(--vr-font-mono);
  font-size: 85%;
  overflow-wrap: anywhere;
}

.body :deep(.rc-codeblock) {
  position: relative;
}

.body :deep(.rc-code) {
  margin: 0;
  padding: var(--vr-space-4);
  overflow: auto;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
  background: var(--rc-code-bg);
  font-family: var(--vr-font-mono);
  font-size: 85%;
  line-height: 1.45;
  tab-size: 4;
}

.body :deep(.rc-code code) {
  padding: 0;
  background: transparent;
  font: inherit;
  white-space: pre;
  overflow-wrap: normal;
}

.body :deep(.rc-lang) {
  position: absolute;
  top: var(--vr-space-1);
  right: 36px;
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  line-height: 24px;
  pointer-events: none;
  user-select: none;
}

.body :deep(.rc-copy) {
  position: absolute;
  top: var(--vr-space-1);
  right: var(--vr-space-1);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
  opacity: 0;
}

.body :deep(.rc-codeblock:hover .rc-copy),
.body :deep(.rc-copy:focus-visible),
.body :deep(.rc-copy[data-state]) {
  opacity: 1;
}

.body :deep(.rc-copy:hover) {
  color: var(--vr-color-text);
}

.body :deep(.rc-copy-idle) {
  display: inline-flex;
}

.body :deep(.rc-copy-done) {
  display: none;
  color: var(--vr-color-success);
}

.body :deep(.rc-copy[data-state='ok'] .rc-copy-done) {
  display: inline-flex;
}

.body :deep(.rc-copy[data-state='ok'] .rc-copy-idle) {
  display: none;
}

.body :deep(.rc-copy[data-state='fail']) {
  color: var(--vr-color-danger);
}

.body :deep(.rc-table) {
  overflow-x: auto;
}

.body :deep(table) {
  width: max-content;
  max-width: none;
  border-collapse: collapse;
  font-size: inherit;
}

.body :deep(th),
.body :deep(td) {
  padding: 6px 13px;
  border: 1px solid var(--vr-color-border);
  text-align: left;
  vertical-align: top;
  overflow-wrap: normal;
}

.body :deep(th[align='center']),
.body :deep(td[align='center']) {
  text-align: center;
}

.body :deep(th[align='right']),
.body :deep(td[align='right']) {
  text-align: right;
}

.body :deep(th) {
  background: var(--vr-color-surface-muted);
  font-weight: 600;
}

.body :deep(tbody tr:nth-child(even)) {
  background: var(--rc-zebra);
}

.body :deep(del) {
  color: var(--vr-color-text-muted);
}

.body :deep(kbd) {
  display: inline-block;
  padding: 3px 5px;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-surface-muted);
  box-shadow: inset 0 -1px 0 var(--vr-color-border);
  font-family: var(--vr-font-mono);
  font-size: 11px;
  line-height: 10px;
  vertical-align: middle;
}

.body :deep(details) {
  padding: var(--vr-space-1) 0;
}

.body :deep(summary) {
  cursor: pointer;
  font-weight: 600;
}

.body :deep(details[open] > summary) {
  margin-bottom: var(--vr-space-2);
}

.body :deep(.rc-alert) {
  --rc-alert: var(--rc-link);
  padding: var(--vr-space-1) var(--vr-space-4);
  border-left: 0.25em solid var(--rc-alert);
  color: var(--vr-color-text);
}

.body :deep(.rc-alert[data-kind='tip']) {
  --rc-alert: var(--vr-color-success);
}

.body :deep(.rc-alert[data-kind='important']) {
  --rc-alert: var(--vr-palette-avatar-6);
}

.body :deep(.rc-alert[data-kind='warning']) {
  --rc-alert: var(--vr-color-conflict);
}

.body :deep(.rc-alert[data-kind='caution']) {
  --rc-alert: var(--vr-color-danger);
}

.body :deep(.rc-alert > :last-child) {
  margin-bottom: 0;
}

.body :deep(.rc-alert-title) {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0 0 var(--vr-space-2);
  color: var(--rc-alert);
  font-weight: 600;
}

.body :deep(.rc-alert-title svg) {
  flex: none;
}

.body :deep(.rc-frontmatter) {
  overflow-x: auto;
}

.body :deep(.rc-frontmatter table) {
  font-size: 0.9em;
}

.body :deep(.rc-frontmatter th) {
  color: var(--vr-color-text-muted);
  font-family: var(--vr-font-mono);
  font-size: 0.9em;
  font-weight: 500;
  white-space: nowrap;
}

.body :deep(.rc-math[data-display='block']) {
  overflow: auto hidden;
  text-align: center;
}

.body :deep(.rc-math:not([data-rendered])) {
  font-family: var(--vr-font-mono);
  font-size: 0.9em;
}

.body :deep(.katex-display) {
  margin: 0;
}

.body :deep(.footnotes) {
  margin-top: var(--vr-space-6);
  padding-top: var(--vr-space-2);
  border-top: 1px solid var(--vr-color-border);
  color: var(--vr-color-text-muted);
  font-size: 0.875em;
}

.body :deep(.footnotes .sr-only) {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}

.body :deep(.footnotes ol) {
  margin-bottom: 0;
}

.body :deep(.footnotes li > p) {
  display: inline;
  margin: 0;
}

.body :deep([data-footnote-backref]) {
  margin-left: 0.3em;
}

.body :deep(sup > a) {
  padding: 0 1px;
}
</style>
