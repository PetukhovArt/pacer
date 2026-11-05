<script setup lang="ts">
import { computed, nextTick, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { ArrowRight, ExternalLink, GitMerge, GitPullRequest, GitPullRequestClosed, GitPullRequestDraft, RefreshCw, Tag, TriangleAlert } from 'lucide-vue-next';
import { VButton } from '@ui/kit';
import {
  describeError,
  forgeChangeRequest,
  forgeDiff,
  forgeDiscussions,
  forgePipeline,
  openUrl,
  type ChangeRequest,
  type ChangeRequestRef,
  type DiffFile,
  type Discussion,
  type Pipeline,
} from '../connectors/api';
import PersonAvatar from '../connectors/PersonAvatar.vue';
import RichContent from '../content/RichContent.vue';
import DiffView from './DiffView.vue';
import { fullDate, numberLabel, PIPELINE_LABEL, relativeTime } from './format';
import PipelineIcon from './PipelineIcon.vue';
import PipelineJobs from './PipelineJobs.vue';
import ThreadView from './ThreadView.vue';

const props = defineProps<{ connector: string; project: string; number: number }>();

type Tab = 'overview' | 'diff' | 'pipeline';

const STATE_LABEL = { open: 'Открыт', merged: 'Влит', closed: 'Закрыт' } as const;

const cr = shallowRef<ChangeRequest | null>(null);
const discussions = shallowRef<Discussion[]>([]);
const pipeline = shallowRef<Pipeline | null>(null);
const files = shallowRef<DiffFile[]>([]);
const loading = shallowRef(false);
const failure = shallowRef<string | null>(null);
const sideFailure = shallowRef<string | null>(null);
const diffLoading = shallowRef(false);
const diffFailure = shallowRef<string | null>(null);
const diffLoaded = shallowRef(false);
const tab = shallowRef<Tab>('overview');
const pipelineOpen = shallowRef(false);
const diffRef = useTemplateRef<InstanceType<typeof DiffView>>('diff');

const target = computed<ChangeRequestRef>(() => ({ connector: props.connector, project: props.project, number: props.number }));
const unresolved = computed(() => discussions.value.filter((d) => d.resolvable && !d.resolved).length);
const conversation = computed(() => discussions.value.filter((d) => d.notes.some((n) => !n.system) || !d.position));
const anchored = computed(() => discussions.value.some((d) => d.position));
let generation = 0;

async function loadDiff() {
  const ticket = generation;
  diffLoading.value = true;
  diffFailure.value = null;
  try {
    const result = await forgeDiff(target.value);
    if (ticket === generation) {
      files.value = result;
      diffLoaded.value = true;
    }
  } catch (e) {
    if (ticket === generation) diffFailure.value = describeError(e);
  } finally {
    if (ticket === generation) diffLoading.value = false;
  }
}

async function load() {
  const ticket = ++generation;
  loading.value = true;
  failure.value = null;
  sideFailure.value = null;
  try {
    const [request, threads, run] = await Promise.allSettled([forgeChangeRequest(target.value), forgeDiscussions(target.value), forgePipeline(target.value)]);
    if (ticket !== generation) return;
    if (request.status === 'rejected') {
      failure.value = describeError(request.reason);
      return;
    }
    cr.value = request.value;
    discussions.value = threads.status === 'fulfilled' ? threads.value : [];
    pipeline.value = run.status === 'fulfilled' ? run.value : null;
    const problems = [threads, run].filter((r): r is PromiseRejectedResult => r.status === 'rejected');
    sideFailure.value = problems.length ? problems.map((p) => describeError(p.reason)).join('; ') : null;
    if (tab.value === 'diff' || diffLoaded.value) await loadDiff();
  } finally {
    if (ticket === generation) loading.value = false;
  }
}

function selectTab(next: Tab) {
  tab.value = next;
  if (next === 'diff' && !diffLoaded.value && !diffLoading.value) void loadDiff();
}

const pendingThread = shallowRef<string | null>(null);

function gotoThread(discussion: Discussion) {
  pendingThread.value = discussion.id;
  selectTab('diff');
}

watch([diffLoaded, tab, pendingThread, diffRef], async () => {
  const id = pendingThread.value;
  if (!id || tab.value !== 'diff' || !diffLoaded.value || !diffRef.value) return;
  pendingThread.value = null;
  await nextTick();
  diffRef.value?.focusThread(id);
});

watch(
  () => [props.connector, props.project, props.number],
  () => {
    cr.value = null;
    discussions.value = [];
    pipeline.value = null;
    files.value = [];
    diffLoaded.value = false;
    diffFailure.value = null;
    pipelineOpen.value = false;
    tab.value = 'overview';
    void load();
  },
);

onMounted(() => void load());
</script>

<template>
  <section class="cr" data-testid="change-request">
    <p v-if="failure" class="failure" role="alert" data-testid="cr-failure">{{ failure }}</p>
    <p v-else-if="!cr" class="loading">Загрузка…</p>

    <template v-if="cr">
      <header class="head">
        <div class="title-row">
          <component :is="cr.state === 'merged' ? GitMerge : cr.state === 'closed' ? GitPullRequestClosed : cr.draft ? GitPullRequestDraft : GitPullRequest" :size="20" class="state-icon" :data-state="cr.state" />
          <h2 class="title">{{ cr.title }} <span class="num">{{ numberLabel(cr) }}</span></h2>
          <VButton size="small" severity="secondary" text :loading="loading" aria-label="Обновить" title="Обновить" data-testid="cr-refresh" @click="load">
            <template #icon><RefreshCw :size="14" /></template>
          </VButton>
          <VButton size="small" severity="secondary" label="Открыть в браузере" data-testid="cr-open-browser" @click="openUrl(cr.url)">
            <template #icon><ExternalLink :size="14" /></template>
          </VButton>
        </div>

        <div class="meta">
          <span class="chip state" :data-state="cr.state" data-testid="cr-state">{{ STATE_LABEL[cr.state] }}</span>
          <span v-if="cr.draft" class="chip" data-testid="cr-draft">Черновик</span>
          <span v-if="cr.mergeable === false" class="chip warn" title="Есть конфликты слияния"><TriangleAlert :size="12" />Конфликты</span>
          <span class="branches" :title="`${cr.source_branch} → ${cr.target_branch}`">
            <code>{{ cr.source_branch }}</code><ArrowRight :size="12" /><code>{{ cr.target_branch }}</code>
          </span>
          <span class="person"><PersonAvatar :person="cr.author" small />{{ cr.author.name || cr.author.username }}</span>
          <span class="muted" :title="fullDate(cr.updated_at)">обновлён {{ relativeTime(cr.updated_at) }}</span>
        </div>

        <div class="meta">
          <span v-if="cr.reviewers.length" class="person" data-testid="cr-reviewers">
            <span class="muted">Ревьюеры:</span>
            <PersonAvatar v-for="r in cr.reviewers" :key="r.username" :person="r" small />
          </span>
          <span v-for="label in cr.labels" :key="label" class="chip label"><Tag :size="10" />{{ label }}</span>
          <button v-if="pipeline" class="chip pipeline" :aria-expanded="pipelineOpen" data-testid="cr-pipeline-chip" @click="pipelineOpen = !pipelineOpen">
            <PipelineIcon :status="pipeline.status" />{{ PIPELINE_LABEL[pipeline.status] }}
          </button>
          <span v-else-if="cr.pipeline !== 'none'" class="chip pipeline"><PipelineIcon :status="cr.pipeline" />{{ PIPELINE_LABEL[cr.pipeline] }}</span>
        </div>

        <div v-if="pipeline && pipelineOpen" class="pipeline-pop" data-testid="cr-pipeline-jobs">
          <a class="link" :href="pipeline.url" @click.prevent="openUrl(pipeline.url)">Пайплайн #{{ pipeline.id }} <ExternalLink :size="12" /></a>
          <PipelineJobs :pipeline="pipeline" />
        </div>
        <p v-if="sideFailure" class="side-failure" role="alert">{{ sideFailure }}</p>
      </header>

      <nav class="tabs" role="tablist">
        <button class="tab" role="tab" :aria-selected="tab === 'overview'" data-testid="cr-tab-overview" @click="selectTab('overview')">
          Обзор<span v-if="unresolved" class="badge" :title="`Нерешённых обсуждений: ${unresolved}`">{{ unresolved }}</span>
        </button>
        <button class="tab" role="tab" :aria-selected="tab === 'diff'" data-testid="cr-tab-diff" @click="selectTab('diff')">
          Изменения<span v-if="diffLoaded" class="badge muted-badge">{{ files.length }}</span>
        </button>
        <button class="tab" role="tab" :aria-selected="tab === 'pipeline'" data-testid="cr-tab-pipeline" @click="selectTab('pipeline')">Пайплайн</button>
      </nav>

      <div v-show="tab === 'overview'" class="pane scroll" data-testid="cr-overview">
        <RichContent v-if="cr.description.trim()" :markdown="cr.description" :connector="connector" />
        <p v-else class="muted">Описание не заполнено</p>

        <h3 v-if="conversation.length" class="section">Обсуждения <span class="count">{{ conversation.length }}</span></h3>
        <div class="threads">
          <ThreadView v-for="d in conversation" :key="d.id" :discussion="d" :connector="connector" show-position @goto="gotoThread" />
        </div>
        <p v-if="anchored" class="hint muted">Клавиши в «Изменениях»: j/k — файлы, n/p — нерешённые обсуждения</p>
      </div>

      <div v-if="tab === 'diff' || diffLoaded" v-show="tab === 'diff'" class="pane" data-testid="cr-diff">
        <p v-if="diffFailure" class="failure" role="alert">{{ diffFailure }}</p>
        <p v-else-if="diffLoading && !diffLoaded" class="loading">Загрузка изменений…</p>
        <DiffView v-else ref="diff" :files="files" :discussions="discussions" :connector="connector" />
      </div>

      <div v-show="tab === 'pipeline'" class="pane scroll" data-testid="cr-pipeline">
        <template v-if="pipeline">
          <p class="pipeline-head">
            <PipelineIcon :status="pipeline.status" :size="16" />{{ PIPELINE_LABEL[pipeline.status] }}
            <a class="link" :href="pipeline.url" @click.prevent="openUrl(pipeline.url)">Открыть <ExternalLink :size="12" /></a>
          </p>
          <PipelineJobs :pipeline="pipeline" />
        </template>
        <p v-else class="muted">У этого запроса нет пайплайна</p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.cr {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.loading,
.muted {
  color: var(--vr-color-text-muted);
}

.loading,
.failure {
  margin: var(--vr-space-4);
}

.failure,
.side-failure {
  color: var(--vr-color-danger);
}

.side-failure {
  margin: 0;
  font-size: var(--vr-font-size-xs);
}

.head {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3) var(--vr-space-4) var(--vr-space-2);
  border-bottom: 1px solid var(--vr-color-border);
}

.title-row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
}

.title {
  flex: 1;
  min-width: 0;
  margin: 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
}

.num {
  color: var(--vr-color-text-muted);
  font-weight: 400;
}

.state-icon {
  flex: none;
  color: var(--vr-color-success);
}

.state-icon[data-state='merged'] {
  color: var(--vr-color-agent-running);
}

.state-icon[data-state='closed'] {
  color: var(--vr-color-danger);
}

.meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-2) var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 1px var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  background: var(--vr-color-chip-bg);
  border: none;
  border-radius: var(--vr-radius-full);
}

.chip.state[data-state='open'] {
  color: var(--vr-color-success);
}

.chip.state[data-state='merged'] {
  color: var(--vr-color-agent-running);
}

.chip.state[data-state='closed'] {
  color: var(--vr-color-danger);
}

.chip.warn {
  color: var(--vr-color-conflict);
}

.chip.label {
  color: var(--vr-color-text);
}

button.chip {
  color: var(--vr-color-text);
  cursor: pointer;
}

.branches {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  color: var(--vr-color-text-muted);
}

code {
  font-family: var(--vr-font-mono);
  color: var(--vr-color-text);
}

.person {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
}

.pipeline-pop {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  max-height: 260px;
  padding: var(--vr-space-2);
  overflow: auto;
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.link {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  align-self: flex-start;
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-xs);
  text-decoration: underline;
  cursor: pointer;
}

.tabs {
  display: flex;
  flex: none;
  gap: var(--vr-space-1);
  padding: 0 var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.tab {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: var(--vr-space-2) var(--vr-space-3);
  color: var(--vr-color-text-muted);
  font: inherit;
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
}

.tab[aria-selected='true'] {
  color: var(--vr-color-text);
  border-bottom-color: var(--vr-color-accent);
}

.badge {
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-conflict);
  font-size: var(--vr-font-size-2xs);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-full);
}

.muted-badge {
  color: var(--vr-color-text-muted);
}

.pane {
  flex: 1;
  min-height: 0;
}

.pane.scroll {
  padding: var(--vr-space-4);
  overflow: auto;
}

.section {
  margin: var(--vr-space-5) 0 var(--vr-space-2);
  font-size: var(--vr-font-size-sm);
}

.count {
  color: var(--vr-color-text-muted);
  font-weight: 400;
}

.threads {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.hint {
  margin: var(--vr-space-4) 0 0;
  font-size: var(--vr-font-size-xs);
}

.pipeline-head {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0 0 var(--vr-space-3);
}
</style>
