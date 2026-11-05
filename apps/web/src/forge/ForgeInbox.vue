<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, watch } from 'vue';
import { GitPullRequest, MessageSquare, RefreshCw, Search, X } from 'lucide-vue-next';
import { VButton, VInputText } from '@ui/kit';
import { useConnectors, type ChangeRequestRef, type InboxError, type InboxItem } from '../connectors/api';
import PersonAvatar from '../connectors/PersonAvatar.vue';
import { fullDate, numberLabel, PIPELINE_LABEL, relativeTime } from './format';
import { useForgeInbox } from './inbox';
import PipelineIcon from './PipelineIcon.vue';

const emit = defineEmits<{ open: [ref: ChangeRequestRef] }>();

const MOUNT_FRESH_MS = 15_000;

const { connectors, list } = useConnectors();
const { items, errors, failure, loading, loadedOnce, loadedAt, load, loadIfStale } = useForgeInbox();

const dismissed = shallowRef<ReadonlySet<string>>(new Set());
const filter = shallowRef('');
const now = shallowRef(Date.now());

let timer: ReturnType<typeof setInterval> | undefined;

const GROUPS = [
  { reason: 'reviewer', title: 'Ждут моего ревью' },
  { reason: 'author', title: 'Мои' },
  { reason: 'assignee', title: 'Назначены на меня' },
] as const;

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

const groups = computed(() => {
  const query = filter.value.trim().toLowerCase();
  const byUpdate = (a: InboxItem, b: InboxItem) => Date.parse(b.updated_at) - Date.parse(a.updated_at);
  return GROUPS.map((group) => {
    const claimed = group.reason === 'reviewer' ? (i: InboxItem) => i.reasons.includes('reviewer') : group.reason === 'author' ? (i: InboxItem) => !i.reasons.includes('reviewer') && i.reasons.includes('author') : (i: InboxItem) => !i.reasons.includes('reviewer') && !i.reasons.includes('author');
    return { ...group, items: items.value.filter((i) => claimed(i) && matches(i, query)).sort(byUpdate) };
  }).filter((group) => group.items.length > 0);
});

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
        <VInputText v-model="filter" class="search-input" placeholder="Фильтр: проект, номер, заголовок, автор" autocomplete="off" data-testid="forge-filter" />
      </div>
      <VButton size="small" severity="secondary" text :loading="loading" aria-label="Обновить" title="Обновить" data-testid="forge-refresh" @click="load">
        <template #icon><RefreshCw :size="14" /></template>
      </VButton>
    </header>

    <div v-if="failure" class="strip" role="alert" data-testid="forge-failure">
      <span>{{ failure }}</span>
    </div>
    <div v-for="e in shownErrors" :key="errorKey(e)" class="strip" role="alert" data-testid="forge-error">
      <span><b>{{ connectorLabel(e.connector) }}</b>: {{ e.message }}</span>
      <button class="strip-close" aria-label="Скрыть" title="Скрыть" @click="dismiss(e)"><X :size="14" /></button>
    </div>

    <div class="scroll">
      <p v-if="!loadedOnce" class="note">Загрузка…</p>
      <p v-else-if="!items.length && !failure" class="note" data-testid="forge-empty"><GitPullRequest :size="16" />Нет открытых MR/PR, связанных с вами</p>
      <p v-else-if="!groups.length" class="note">Ничего не найдено</p>

      <section v-for="group in groups" :key="group.reason" class="group" :data-testid="`forge-group-${group.reason}`">
        <h3>{{ group.title }} <span class="count">{{ group.items.length }}</span></h3>
        <button v-for="item in group.items" :key="`${item.connector}/${item.project}/${item.number}`" class="row" data-testid="forge-row" @click="open(item)">
          <PersonAvatar :person="item.author" small />
          <span class="main">
            <span class="title">
              <span v-if="item.draft" class="draft" data-testid="forge-draft">Черновик</span>{{ item.title }}
            </span>
            <span class="sub">{{ item.project }} · {{ numberLabel(item) }} · {{ item.source_branch }}</span>
          </span>
          <span v-if="item.unresolved_threads" class="threads" :title="`Нерешённых обсуждений: ${item.unresolved_threads}`">
            <MessageSquare :size="12" />{{ item.unresolved_threads }}
          </span>
          <PipelineIcon v-if="item.pipeline !== 'none'" :status="item.pipeline" :title="PIPELINE_LABEL[item.pipeline]" />
          <span class="time" :title="fullDate(item.updated_at)">{{ relativeTime(item.updated_at, now) }}</span>
        </button>
      </section>
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
  padding: var(--vr-space-2) var(--vr-space-3) var(--vr-space-4);
  overflow: auto;
}

.note {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: var(--vr-space-4) 0;
  color: var(--vr-color-text-muted);
}

h3 {
  margin: var(--vr-space-3) 0 var(--vr-space-1);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  text-transform: uppercase;
}

.count {
  margin-left: var(--vr-space-1);
  padding: 0 var(--vr-space-2);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  width: 100%;
  min-height: 44px;
  padding: var(--vr-space-1) var(--vr-space-2);
  color: inherit;
  font: inherit;
  text-align: left;
  background: none;
  border: none;
  border-radius: var(--vr-radius-md);
  cursor: pointer;
}

.row:hover,
.row:focus-visible {
  background: var(--vr-color-nav-hover);
  outline: none;
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

.time {
  flex: none;
  min-width: 72px;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  text-align: right;
}
</style>
