<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, watch } from 'vue';
import { GitPullRequest, MessageSquare, RefreshCw, Search, X } from 'lucide-vue-next';
import { VButton, VInputText } from '@ui/kit';
import { useConnectors, type ChangeRequestRef, type InboxError, type InboxItem } from '../connectors/api';
import PersonAvatar from '../connectors/PersonAvatar.vue';
import { t } from '../i18n';
import ExplorerTree from '../tree/ExplorerTree.vue';
import { fullDate, numberLabel, pipelineLabel, relativeTime } from './format';
import { inboxTree, itemId, useForgeInbox, type InboxNode } from './inbox';
import PipelineIcon from './PipelineIcon.vue';

const props = defineProps<{ selected?: ChangeRequestRef }>();
const emit = defineEmits<{ open: [ref: ChangeRequestRef] }>();

const MOUNT_FRESH_MS = 15_000;

const { connectors, list } = useConnectors();
const { items, errors, failure, loading, loadedOnce, loadedAt, load, loadIfStale } = useForgeInbox();

const dismissed = shallowRef<ReadonlySet<string>>(new Set());
const filter = shallowRef('');
const now = shallowRef(Date.now());

let timer: ReturnType<typeof setInterval> | undefined;

const errorKey = (e: InboxError) => `${e.connector}\n${e.message}`;
const connectorLabel = (id: string) => connectors.value.find((c) => c.id === id)?.label ?? id;
const shownErrors = computed(() => errors.value.filter((e) => !dismissed.value.has(errorKey(e))));

function dismiss(error: InboxError) {
  dismissed.value = new Set([...dismissed.value, errorKey(error)]);
}

function matches(item: InboxItem, query: string): boolean {
  if (!query) return true;
  const haystack = `${item.project} ${numberLabel(item)} ${item.number} ${item.title} ${item.author.name} ${item.author.username} ${item.source_branch} ${item.target_branch}`.toLowerCase();
  return query.split(/\s+/).every((word) => haystack.includes(word));
}

const query = computed(() => filter.value.trim().toLowerCase());
const nodes = computed(() => inboxTree(items.value.filter((item) => matches(item, query.value)), connectorLabel));
const selectedId = computed(() => (props.selected ? itemId(props.selected) : null));

const shownTitle = (item: InboxItem) => (item.draft ? item.title.replace(/^(draft:|\[draft\]|\(draft\)|wip:)\s*/i, '') : item.title);

function onOpen(node: InboxNode) {
  if (node.data?.item) open(node.data.item);
}

watch(errors, (list) => {
  const live = new Set(list.map(errorKey));
  dismissed.value = new Set([...dismissed.value].filter((key) => live.has(key)));
});
watch(loadedAt, (at) => (now.value = at));

onMounted(() => {
  if (!connectors.value.length) void list();
  loadIfStale(MOUNT_FRESH_MS);
  timer = setInterval(() => (now.value = Date.now()), 30_000);
});

onBeforeUnmount(() => clearInterval(timer));

const open = (item: InboxItem) => emit('open', { connector: item.connector, project: item.project, number: item.number });
</script>

<template>
  <section class="inbox" data-testid="forge-inbox">
    <header class="bar">
      <div class="search">
        <Search :size="14" />
        <VInputText v-model="filter" class="search-input" :placeholder="t('forge.inbox.filter')" autocomplete="off" data-testid="forge-filter" />
      </div>
      <VButton size="small" severity="secondary" text :loading="loading" :aria-label="t('common.refresh')" :title="t('common.refresh')" data-testid="forge-refresh" @click="load">
        <template #icon><RefreshCw :size="14" /></template>
      </VButton>
    </header>

    <div v-if="failure" class="strip" role="alert" data-testid="forge-failure">
      <span>{{ failure }}</span>
    </div>
    <div v-for="e in shownErrors" :key="errorKey(e)" class="strip" role="alert" data-testid="forge-error">
      <span><b>{{ connectorLabel(e.connector) }}</b>: {{ e.message }}</span>
      <button class="strip-close" :aria-label="t('forge.inbox.dismiss')" :title="t('forge.inbox.dismiss')" @click="dismiss(e)"><X :size="14" /></button>
    </div>

    <div class="scroll">
      <p v-if="!loadedOnce" class="note">{{ t('common.loading') }}</p>
      <p v-else-if="!items.length && !failure" class="note" data-testid="forge-empty"><GitPullRequest :size="16" />{{ t('forge.inbox.empty') }}</p>
      <p v-else-if="!nodes.length" class="note">{{ t('common.noResults') }}</p>

      <ExplorerTree
        v-if="nodes.length"
        :nodes="nodes"
        storage-key="veyra.reviews.tree.expanded"
        :label="t('forge.inbox.tree')"
        :selected="selectedId"
        :reveal-all="query.length > 0"
        test-id="forge-tree"
        @open="onOpen"
      >
        <template #label="{ node }">
          <span v-if="node.kind === 'project'" class="label path" data-testid="forge-project">
            <span v-if="node.data?.group" class="path-group">{{ node.data.group }}/</span><span class="path-name">{{ node.data?.name ?? node.label }}</span>
          </span>
          <span v-else class="label" :data-kind="node.kind">{{ node.label }}</span>
        </template>
        <template #rollup="{ node }">
          <PipelineIcon v-if="node.data?.pipeline" :status="node.data.pipeline" :size="12" :title="pipelineLabel(node.data.pipeline)" />
        </template>
        <template #leaf="{ data }">
          <span v-if="data?.item" class="row" data-testid="forge-row">
            <PersonAvatar :person="data.item.author" small />
            <span class="main">
              <span class="title">
                <span v-if="data.item.draft" class="draft" data-testid="forge-draft">{{ t('forge.draft') }}</span>{{ shownTitle(data.item) }}
              </span>
              <span class="meta">
                <span class="sub">{{ numberLabel(data.item) }} · {{ data.item.source_branch }}</span>
                <span v-if="data.item.unresolved_threads" class="threads" :title="t('forge.unresolvedThreads', { count: data.item.unresolved_threads })">
                  <MessageSquare :size="12" />{{ data.item.unresolved_threads }}
                </span>
                <PipelineIcon v-if="data.item.pipeline !== 'none'" :status="data.item.pipeline" :size="12" :title="pipelineLabel(data.item.pipeline)" />
                <span class="time" :title="fullDate(data.item.updated_at)">{{ relativeTime(data.item.updated_at, now) }}</span>
              </span>
            </span>
          </span>
        </template>
      </ExplorerTree>
    </div>
  </section>
</template>

<style scoped>
.inbox {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-2) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.search {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-2);
  color: var(--vr-color-text-muted);
}

.search-input {
  flex: 1;
}

.strip {
  display: flex;
  flex: none;
  align-items: center;
  justify-content: space-between;
  gap: var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-3);
  color: var(--vr-color-danger);
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-chip-bg);
  border-bottom: 1px solid var(--vr-color-border);
}

.strip-close {
  display: inline-flex;
  padding: 2px;
  color: inherit;
  background: none;
  border: none;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.scroll {
  flex: 1;
  min-height: 0;
  padding: var(--vr-space-2);
  overflow: auto;
}

.note {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: var(--vr-space-4) var(--vr-space-2);
  color: var(--vr-color-text-muted);
}

.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.label[data-kind='section'] {
  font-weight: 600;
}

.label[data-kind='host'] {
  font-weight: 500;
}

.path-group {
  color: var(--vr-color-text-muted);
}

.path-name {
  font-weight: 600;
}

.row {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  min-height: 40px;
  padding: 2px 0;
}

.main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.title,
.sub {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.sub {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.draft {
  margin-right: var(--vr-space-2);
  padding: 1px var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.threads {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 2px;
  color: var(--vr-color-conflict);
  font-size: var(--vr-font-size-xs);
}

.meta {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.sub {
  flex: 1;
  min-width: 0;
}

.time {
  flex: none;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}
</style>
