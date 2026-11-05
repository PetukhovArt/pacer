<script setup lang="ts">
import { computed, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { openUrl } from '../connectors/api';
import { loadConnectorImages, resolveAgainst } from './assets';
import MermaidView from './MermaidView.vue';
import { renderRich } from './render';

type Diagram = { key: string; el: HTMLElement; source: string };

const props = defineProps<{ markdown?: string; html?: string; connector?: string }>();

const body = useTemplateRef<HTMLElement>('body');
const diagrams = shallowRef<Diagram[]>([]);
const rendered = computed(() => renderRich({ markdown: props.markdown, html: props.html }));

let generation = 0;

function sync() {
  const el = body.value;
  if (!el) return;
  const current = ++generation;
  const sources = rendered.value.diagrams;
  diagrams.value = [...el.querySelectorAll<HTMLElement>('[data-rc-mermaid]')].map((target, index) => ({
    key: `${current}-${index}`,
    el: target,
    source: sources[Number(target.dataset.rcMermaid)] ?? '',
  }));
  if (props.connector) void loadConnectorImages(el, props.connector, () => current === generation);
}

function jumpTo(id: string) {
  const el = body.value;
  if (!el || !id) return;
  const target = el.querySelector(`[id="${CSS.escape(id)}"], a[name="${CSS.escape(id)}"]`);
  target?.scrollIntoView({ block: 'start' });
}

async function onClick(event: MouseEvent) {
  if (!(event.target instanceof Element)) return;
  const anchor = event.target.closest('a[href]');
  if (!anchor || !body.value?.contains(anchor)) return;
  const raw = anchor.getAttribute('href') ?? '';
  event.preventDefault();
  if (raw.startsWith('#')) {
    jumpTo(decodeURIComponent(raw.slice(1)));
    return;
  }
  const url = await resolveAgainst(props.connector, raw);
  if (url && /^(?:https?|mailto):/i.test(url)) await openUrl(url);
}

watch([rendered, () => props.connector], sync, { flush: 'post' });
onMounted(sync);
</script>

<template>
  <div class="rich-content" data-testid="rich-content">
    <div ref="body" class="body" @click="onClick" v-html="rendered.html" />
    <Teleport v-for="diagram in diagrams" :key="diagram.key" :to="diagram.el">
      <MermaidView :source="diagram.source" />
    </Teleport>
  </div>
</template>

<style scoped>
.rich-content {
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  line-height: 1.7;
  overflow-wrap: anywhere;
}

.body > :deep(:first-child) {
  margin-top: 0;
}

.body > :deep(:last-child) {
  margin-bottom: 0;
}

.body :deep(p),
.body :deep(ul),
.body :deep(ol),
.body :deep(dl),
.body :deep(blockquote),
.body :deep(h1),
.body :deep(h2),
.body :deep(h3),
.body :deep(h4),
.body :deep(h5),
.body :deep(h6) {
  max-width: 80ch;
}

.body :deep(p),
.body :deep(ul),
.body :deep(ol),
.body :deep(dl),
.body :deep(blockquote),
.body :deep(.rc-table),
.body :deep(.rc-code),
.body :deep(.rc-diagram) {
  margin: 0 0 var(--vr-space-4);
}

.body :deep(h1),
.body :deep(h2),
.body :deep(h3),
.body :deep(h4),
.body :deep(h5),
.body :deep(h6) {
  margin: var(--vr-space-6) 0 var(--vr-space-3);
  line-height: 1.3;
  font-weight: 650;
  scroll-margin-top: var(--vr-space-4);
}

.body :deep(h1) {
  padding-bottom: var(--vr-space-2);
  border-bottom: 1px solid var(--vr-color-border);
  font-size: calc(var(--vr-font-size-xl) + 4px);
}

.body :deep(h2) {
  padding-bottom: var(--vr-space-1);
  border-bottom: 1px solid var(--vr-color-border);
  font-size: var(--vr-font-size-xl);
}

.body :deep(h3) {
  font-size: var(--vr-font-size-lg);
}

.body :deep(h4) {
  font-size: var(--vr-font-size-base);
}

.body :deep(h5),
.body :deep(h6) {
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text-muted);
}

.body :deep(ul),
.body :deep(ol) {
  padding-left: var(--vr-space-6);
}

.body :deep(ul ul),
.body :deep(ul ol),
.body :deep(ol ul),
.body :deep(ol ol) {
  margin: var(--vr-space-1) 0;
}

.body :deep(li + li) {
  margin-top: var(--vr-space-1);
}

.body :deep(li > p) {
  margin: 0 0 var(--vr-space-1);
}

.body :deep(.rc-task) {
  margin-left: calc(var(--vr-space-5) * -1);
  list-style: none;
}

.body :deep(.rc-task input) {
  margin: 0 var(--vr-space-2) 0 0;
  vertical-align: middle;
}

.body :deep(blockquote) {
  padding: var(--vr-space-1) var(--vr-space-4);
  border-left: 3px solid var(--vr-color-border);
  color: var(--vr-color-text-muted);
}

.body :deep(blockquote > :last-child) {
  margin-bottom: 0;
}

.body :deep(a) {
  color: var(--vr-color-accent);
  text-decoration: underline;
  text-underline-offset: 2px;
  text-decoration-color: var(--vr-color-text-muted);
}

.body :deep(a:hover) {
  text-decoration-color: currentColor;
}

.body :deep(hr) {
  margin: var(--vr-space-6) 0;
  border: 0;
  border-top: 1px solid var(--vr-color-border);
}

.body :deep(img) {
  max-width: 100%;
  height: auto;
  border-radius: var(--vr-radius-sm);
}

.body :deep(:not(pre) > code) {
  padding: 1px var(--vr-space-1);
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-surface-muted);
  font-family: var(--vr-font-mono);
  font-size: 0.9em;
}

.body :deep(.rc-code) {
  position: relative;
  padding: var(--vr-space-3) var(--vr-space-4);
  overflow: auto;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  background: var(--vr-color-surface);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  line-height: 1.6;
  tab-size: 4;
}

.body :deep(.rc-code code) {
  padding: 0;
  background: transparent;
  font: inherit;
  white-space: pre;
  overflow-wrap: normal;
}

.body :deep(.rc-code[data-lang])::after {
  content: attr(data-lang);
  position: absolute;
  top: var(--vr-space-1);
  right: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  pointer-events: none;
}

.body :deep(.rc-table) {
  overflow-x: auto;
}

.body :deep(table) {
  border-collapse: collapse;
  font-size: var(--vr-font-size-sm);
}

.body :deep(th),
.body :deep(td) {
  padding: var(--vr-space-1) var(--vr-space-3);
  border: 1px solid var(--vr-color-border);
  text-align: left;
  vertical-align: top;
}

.body :deep(th) {
  background: var(--vr-color-surface-muted);
  font-weight: 600;
}

.body :deep(tbody tr:nth-child(even)) {
  background: var(--vr-palette-white-a-4);
}

.body :deep(del) {
  color: var(--vr-color-text-muted);
}

.body :deep(kbd) {
  padding: 0 var(--vr-space-1);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
  font-family: var(--vr-font-mono);
  font-size: 0.85em;
}
</style>
