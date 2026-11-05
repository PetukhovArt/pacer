<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';
import { VButton } from '@ui/kit';
import RichContent from '../content/RichContent.vue';
import { t } from '../i18n';
import { confluencePage, describeError, openUrl, type ConfluencePage } from '../connectors/api';
import ErrorInline from './ErrorInline.vue';
import Skeleton from './Skeleton.vue';
import { rememberPage } from './util';

type Heading = { index: number; text: string; level: number };

const props = defineProps<{ connector: string; id: string }>();
const emit = defineEmits<{ open: [payload: { connector: string; id: string }] }>();

const page = ref<ConfluencePage | null>(null);
const loading = ref(false);
const error = ref('');
const headings = ref<Heading[]>([]);
const activeIndex = ref(0);
const scroller = ref<HTMLElement | null>(null);
const bodyEl = ref<HTMLElement | null>(null);
let elements: HTMLElement[] = [];
let observer: MutationObserver | undefined;
let frame = 0;
let seq = 0;

async function load() {
  const id = ++seq;
  loading.value = true;
  error.value = '';
  page.value = null;
  headings.value = [];
  elements = [];
  try {
    const result = await confluencePage(props.connector, props.id);
    if (id !== seq) return;
    page.value = result;
    rememberPage({ connector: result.connector, id: result.id, title: result.title, space: result.space });
  } catch (e) {
    if (id === seq) error.value = describeError(e);
  } finally {
    if (id === seq) loading.value = false;
  }
}

function collect() {
  const found = bodyEl.value ? Array.from(bodyEl.value.querySelectorAll<HTMLElement>('h1, h2, h3, h4')).filter((el) => el.textContent?.trim()) : [];
  elements = found;
  headings.value = found.map((el, index) => ({ index, text: el.textContent!.trim(), level: Number(el.tagName[1]) }));
  spy();
}

function observe() {
  observer?.disconnect();
  if (!bodyEl.value) return;
  observer = new MutationObserver(() => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(collect);
  });
  observer.observe(bodyEl.value, { childList: true, subtree: true });
  collect();
}

function spy() {
  const top = scroller.value?.getBoundingClientRect().top ?? 0;
  let active = 0;
  elements.forEach((el, index) => {
    if (el.getBoundingClientRect().top - top <= 24) active = index;
  });
  activeIndex.value = active;
}

function jump(index: number) {
  elements[index]?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  activeIndex.value = index;
}

watch(bodyEl, (el) => (el ? observe() : observer?.disconnect()), { flush: 'post' });
watch(() => [props.connector, props.id], load, { immediate: true });
onBeforeUnmount(() => {
  observer?.disconnect();
  cancelAnimationFrame(frame);
});
</script>

<template>
  <div ref="scroller" class="page" data-testid="confluence-page" @scroll.passive="spy">
    <ErrorInline v-if="error" :message="error" @retry="load" />
    <Skeleton v-else-if="loading" :rows="12" />
    <template v-else-if="page">
      <header class="head">
        <nav v-if="page.ancestors.length" class="crumbs" :aria-label="t('confluence.page.path')">
          <template v-for="ancestor in page.ancestors" :key="ancestor.id">
            <button class="crumb" @click="emit('open', { connector: connector, id: ancestor.id })">{{ ancestor.title }}</button>
            <span class="sep">/</span>
          </template>
        </nav>
        <h1 class="title">{{ page.title }}</h1>
        <div class="meta">
          <span class="space">{{ page.space }}</span>
          <span class="muted">{{ t('confluence.page.version', { version: page.version }) }}</span>
          <span class="spacer" />
          <VButton :label="t('common.openInBrowser')" size="small" severity="secondary" @click="openUrl(page.url)" />
        </div>
      </header>
      <div class="layout">
        <div ref="bodyEl" class="content">
          <RichContent :html="page.body_html" :connector="page.connector" />
        </div>
        <nav v-if="headings.length" class="toc" :aria-label="t('confluence.page.contents')">
          <div class="toc-title">{{ t('confluence.page.contents') }}</div>
          <button
            v-for="heading in headings"
            :key="heading.index"
            class="toc-item"
            :class="{ active: heading.index === activeIndex }"
            :style="{ paddingLeft: `${(heading.level - 1) * 12 + 8}px` }"
            :title="heading.text"
            @click="jump(heading.index)"
          >
            {{ heading.text }}
          </button>
        </nav>
      </div>
    </template>
  </div>
</template>

<style scoped>
.page {
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: auto;
  background: var(--vr-color-surface);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.head {
  padding: var(--vr-space-4) var(--vr-space-6) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.crumbs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  margin-bottom: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
}

.crumb {
  padding: 0 var(--vr-space-1);
  border: 0;
  border-radius: var(--vr-radius-sm);
  background: transparent;
  color: var(--vr-color-text-muted);
  font: inherit;
  cursor: pointer;
}

.crumb:hover {
  background: var(--vr-color-nav-hover);
  color: var(--vr-color-text);
}

.sep {
  color: var(--vr-color-text-muted);
}

.title {
  margin: 0 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xl);
  font-weight: 600;
  overflow-wrap: anywhere;
}

.meta {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
}

.space {
  padding: 0 var(--vr-space-2);
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-chip-bg);
}

.muted {
  color: var(--vr-color-text-muted);
}

.spacer {
  flex: 1;
}

.layout {
  display: flex;
  align-items: flex-start;
  gap: var(--vr-space-6);
  padding: var(--vr-space-4) var(--vr-space-6);
}

.content {
  flex: 1;
  min-width: 0;
}

.toc {
  position: sticky;
  top: var(--vr-space-4);
  display: flex;
  flex: none;
  flex-direction: column;
  width: 220px;
  max-height: calc(100vh - 160px);
  overflow: auto;
  border-left: 1px solid var(--vr-color-border);
}

.toc-title {
  padding: 0 var(--vr-space-2) var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  text-transform: uppercase;
}

.toc-item {
  overflow: hidden;
  padding-top: var(--vr-space-1);
  padding-right: var(--vr-space-2);
  padding-bottom: var(--vr-space-1);
  border: 0;
  border-left: 2px solid transparent;
  background: transparent;
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.toc-item:hover {
  color: var(--vr-color-text);
}

.toc-item.active {
  border-left-color: var(--vr-color-accent);
  color: var(--vr-color-text);
}
</style>
