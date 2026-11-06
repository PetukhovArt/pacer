<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { forgeForWorktree, type ChangeRequestRef, type ChangeRequestSummary } from '../connectors/api';
import { t } from '../i18n';
import { numberLabel, pipelineLabel } from './format';
import PipelineIcon from './PipelineIcon.vue';

const props = defineProps<{ path: string; branch: string; summary?: ChangeRequestSummary }>();
const emit = defineEmits<{ open: [ref: ChangeRequestRef] }>();

const TTL_MS = 60_000;
const cache = new Map<string, { at: number; value: Promise<ChangeRequestSummary | null> }>();

function lookup(path: string, branch: string): Promise<ChangeRequestSummary | null> {
  const key = `${path}\0${branch}`;
  const hit = cache.get(key);
  if (hit && Date.now() - hit.at < TTL_MS) return hit.value;
  const value = forgeForWorktree(path, branch).catch(() => null);
  cache.set(key, { at: Date.now(), value });
  return value;
}

const looked = shallowRef<ChangeRequestSummary | null>(null);
const summary = computed(() => props.summary ?? looked.value);

watch(
  () => [props.path, props.branch, props.summary] as const,
  async ([path, branch, known]) => {
    looked.value = null;
    if (known || !path || !branch) return;
    const result = await lookup(path, branch);
    if (path === props.path && branch === props.branch) looked.value = result;
  },
  { immediate: true },
);

const title = computed(() => {
  const s = summary.value;
  if (!s) return '';
  const parts = [`${numberLabel(s)} ${s.title}`, s.project, pipelineLabel(s.pipeline)];
  if (s.unresolved_threads) parts.push(t('forge.unresolvedThreads', { count: s.unresolved_threads }));
  return parts.join('\n');
});

function open() {
  const s = summary.value;
  if (s) emit('open', { connector: s.connector, project: s.project, number: s.number });
}
</script>

<template>
  <button v-if="summary" class="badge" :title="title" :aria-label="title" data-testid="worktree-badge" @click.stop="open">
    <span class="num">{{ numberLabel(summary) }}</span>
    <PipelineIcon v-if="summary.pipeline !== 'none'" :status="summary.pipeline" :size="12" />
  </button>
</template>

<style scoped>
.badge {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 3px;
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  line-height: 16px;
  background: var(--vr-color-chip-bg);
  border: none;
  border-radius: var(--vr-radius-full);
  cursor: pointer;
}

.badge:hover {
  color: var(--vr-color-text);
}
</style>
