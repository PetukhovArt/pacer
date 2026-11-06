<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, useTemplateRef } from 'vue';
import { ExternalLink, LoaderCircle, RefreshCw, TriangleAlert } from 'lucide-vue-next';
import { VButton } from '@ui/kit';
import { describeError, openUrl } from '../connectors/api';
import { t } from '../i18n';
import { architectureProgress, architectureRelease, architectureStart, type ArchitectureParams, type Progress, type Visualizer } from './stratum';

const props = defineProps<ArchitectureParams>();

const PHASES = ['queued', 'fetch', 'worktree', 'start', 'ready', 'failed'] as const;
const POLL_MS = 600;

const running = shallowRef<Visualizer | null>(null);
const failure = shallowRef<string | null>(null);
const progress = shallowRef<Progress>({ phase: 'queued', lines: [] });
const loading = shallowRef(false);
const frame = useTemplateRef<HTMLIFrameElement>('frame');
let disposed = false;
let ticket = 0;
let poller: ReturnType<typeof setInterval> | undefined;

const phaseLabel = computed(() => {
  const phase = PHASES.find((name) => name === progress.value.phase) ?? 'queued';
  return t(`forge.stratum.phase.${phase}`);
});

function stopPolling() {
  clearInterval(poller);
  poller = undefined;
}

async function poll(current: number) {
  try {
    const next = await architectureProgress(props.connector, props.project, props.number);
    if (current === ticket && loading.value) progress.value = next;
  } catch {
    return;
  }
}

async function start() {
  const current = ++ticket;
  const previous = running.value;
  running.value = null;
  failure.value = null;
  loading.value = true;
  progress.value = { phase: 'queued', lines: [] };
  if (previous) void architectureRelease(previous.key).catch(() => undefined);
  stopPolling();
  poller = setInterval(() => void poll(current), POLL_MS);
  try {
    const found = await architectureStart({ connector: props.connector, project: props.project, number: props.number, repo: props.repo, target: props.target });
    if (disposed || current !== ticket) {
      void architectureRelease(found.key).catch(() => undefined);
      return;
    }
    running.value = found;
  } catch (e) {
    if (current === ticket) {
      failure.value = describeError(e);
      await poll(current);
    }
  } finally {
    if (current === ticket) {
      loading.value = false;
      stopPolling();
    }
  }
}

function focusFrame() {
  frame.value?.focus();
}

onMounted(() => void start());

onBeforeUnmount(() => {
  disposed = true;
  stopPolling();
  if (running.value) void architectureRelease(running.value.key).catch(() => undefined);
});
</script>

<template>
  <section class="arch" data-testid="architecture-view">
    <header class="bar">
      <span class="hint" data-testid="architecture-hint">{{ t('forge.stratum.hint') }}</span>
      <VButton size="small" severity="secondary" text :loading="loading" :aria-label="t('forge.stratum.retry')" :title="t('forge.stratum.retry')" data-testid="architecture-reload" @click="start">
        <template #icon><RefreshCw :size="14" /></template>
      </VButton>
      <VButton v-if="running" size="small" severity="secondary" :label="t('common.openInBrowser')" data-testid="architecture-open-browser" @click="openUrl(running.url)">
        <template #icon><ExternalLink :size="14" /></template>
      </VButton>
    </header>

    <iframe v-if="running" ref="frame" class="frame" :src="running.url" :title="t('forge.stratum.frame')" data-testid="architecture-frame" @load="focusFrame" />

    <div v-else class="state" :role="failure ? 'alert' : 'status'" data-testid="architecture-state">
      <p v-if="failure" class="failure"><TriangleAlert :size="16" />{{ t('forge.stratum.failed') }}</p>
      <p v-else class="phase" data-testid="architecture-phase"><LoaderCircle :size="16" class="spin" />{{ phaseLabel }}</p>
      <pre v-if="failure" class="log failure-text" data-testid="architecture-error">{{ failure }}</pre>
      <pre v-else-if="progress.lines.length" class="log" data-testid="architecture-log">{{ progress.lines.slice(-12).join('\n') }}</pre>
      <VButton v-if="failure" size="small" severity="secondary" :label="t('forge.stratum.retry')" data-testid="architecture-retry" @click="start" />
    </div>
  </section>
</template>

<style scoped>
.arch {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
}

.bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.hint {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.frame {
  flex: 1;
  width: 100%;
  min-height: 0;
  background: var(--vr-color-surface);
  border: 0;
}

.state {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--vr-space-2);
  padding: var(--vr-space-4);
}

.phase,
.failure {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0;
}

.failure {
  color: var(--vr-color-danger);
}

.log {
  max-width: 100%;
  max-height: 260px;
  margin: 0;
  padding: var(--vr-space-2);
  overflow: auto;
  font-family: var(--vr-font-mono, monospace);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: pre-wrap;
  background: var(--vr-color-nav-hover);
  border-radius: var(--vr-radius-sm);
}

.failure-text {
  color: var(--vr-color-danger);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
