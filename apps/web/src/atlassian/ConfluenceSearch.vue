<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { VInputText } from '@ui/kit';
import { confluenceSearch, describeError, reloadWhenRecovered, type ConfluenceHit } from '../connectors/api';
import { t, useI18n } from '../i18n';
import ErrorInline from './ErrorInline.vue';
import Skeleton from './Skeleton.vue';
import { highlightHtml, recentPages, rememberPage, type RecentPage } from './util';

const props = defineProps<{ connector: string }>();
const emit = defineEmits<{ open: [payload: { connector: string; id: string }] }>();

const { formatRelative, formatDate } = useI18n();

const LIMIT = 25;
const DEBOUNCE = 350;

const text = ref('');
const hits = ref<ConfluenceHit[]>([]);
const loading = ref(false);
const error = ref('');
const searched = ref('');
const recent = ref<RecentPage[]>(recentPages(props.connector));
const input = ref<{ $el: HTMLInputElement } | null>(null);
let timer: ReturnType<typeof setTimeout> | undefined;
let seq = 0;

async function run() {
  const query = text.value.trim();
  const id = ++seq;
  if (!query) {
    hits.value = [];
    searched.value = '';
    error.value = '';
    loading.value = false;
    return;
  }
  loading.value = true;
  error.value = '';
  try {
    const result = await confluenceSearch(props.connector, query, LIMIT);
    if (id !== seq) return;
    hits.value = result;
    searched.value = query;
  } catch (e) {
    if (id === seq) error.value = describeError(e);
  } finally {
    if (id === seq) loading.value = false;
  }
}

watch(text, () => {
  clearTimeout(timer);
  timer = setTimeout(run, DEBOUNCE);
});

watch(
  () => props.connector,
  () => {
    recent.value = recentPages(props.connector);
    void run();
  },
);

function openPage(page: RecentPage) {
  rememberPage(page);
  recent.value = recentPages(props.connector);
  emit('open', { connector: props.connector, id: page.id });
}

function openHit(hit: ConfluenceHit) {
  openPage({ connector: hit.connector, id: hit.id, title: hit.title, space: hit.space });
}

function onKey(event: KeyboardEvent) {
  if (event.key === 'Enter') {
    clearTimeout(timer);
    void run();
  }
}

reloadWhenRecovered(
  () => props.connector,
  () => error.value !== '',
  () => void run(),
);

onMounted(() => input.value?.$el.focus());
onBeforeUnmount(() => clearTimeout(timer));
</script>

<template>
  <div class="search" data-testid="confluence-search">
    <div class="bar">
      <VInputText ref="input" v-model="text" class="field" :placeholder="t('confluence.search.placeholder')" :aria-label="t('confluence.search.placeholder')" data-testid="confluence-query" @keydown="onKey" />
    </div>
    <div class="body">
      <ErrorInline v-if="error" :message="error" @retry="run" />
      <Skeleton v-else-if="loading && !hits.length" :rows="8" :height="44" />
      <template v-else-if="searched">
        <div v-if="!hits.length" class="empty">{{ t('confluence.search.noHits', { query: searched }) }}</div>
        <ul v-else v-auto-animate class="list">
          <li v-for="hit in hits" :key="hit.id">
            <button class="hit" data-testid="confluence-hit" @click="openHit(hit)">
              <span class="title">{{ hit.title }}</span>
              <span class="meta">
                <span class="space">{{ hit.space }}</span>
                <span :title="formatDate(hit.updated)">{{ formatRelative(hit.updated) }}</span>
              </span>
              <span v-if="hit.excerpt" class="excerpt" v-html="highlightHtml(hit.excerpt)" />
            </button>
          </li>
        </ul>
      </template>
      <template v-else>
        <div class="group-title">{{ t('confluence.search.recent') }}</div>
        <div v-if="!recent.length" class="empty">{{ t('confluence.search.prompt') }}</div>
        <ul v-else v-auto-animate class="list">
          <li v-for="page in recent" :key="page.id">
            <button class="hit" @click="openPage(page)">
              <span class="title">{{ page.title }}</span>
              <span class="meta"><span class="space">{{ page.space }}</span></span>
            </button>
          </li>
        </ul>
      </template>
    </div>
  </div>
</template>

<style scoped>
.search {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  background: var(--vr-color-surface);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.bar {
  flex: none;
  padding: var(--vr-space-3) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.field {
  width: 100%;
}

.body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.group-title {
  padding: var(--vr-space-3) var(--vr-space-4) var(--vr-space-1);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  text-transform: uppercase;
}

.empty {
  padding: var(--vr-space-6);
  color: var(--vr-color-text-muted);
  text-align: center;
}

.list {
  margin: 0;
  padding: var(--vr-space-2);
  list-style: none;
}

.hit {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: var(--vr-space-1) var(--vr-space-3);
  width: 100%;
  padding: var(--vr-space-2) var(--vr-space-3);
  border: 0;
  border-radius: var(--vr-radius-md);
  background: transparent;
  color: var(--vr-color-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.hit:hover,
.hit:focus-visible {
  background: var(--vr-color-nav-hover);
  outline: none;
}

.title {
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  display: flex;
  gap: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  white-space: nowrap;
}

.space {
  padding: 0 var(--vr-space-2);
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-chip-bg);
  color: var(--vr-color-text);
}

.excerpt {
  display: -webkit-box;
  grid-column: 1 / -1;
  overflow: hidden;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.excerpt :deep(mark) {
  background: var(--vr-color-pin-bg);
  color: var(--vr-color-conflict);
}
</style>
