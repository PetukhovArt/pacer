<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef } from 'vue';
import { useI18n } from '../../i18n';
import { describeError, sessionId, sessionKey, usePacer } from '../pacer';
import { getMetrics, type MetricsSnapshot, type SessionMetrics } from './metrics';

const POLL_INTERVAL_MS = 3000;

const { t, formatBytes } = useI18n();

type Row = { id: string; name: string; procs: number; rssBytes: number; share: number };

const pacer = usePacer();
const snapshot = shallowRef<MetricsSnapshot | null>(null);
const error = shallowRef<string | null>(null);
let timer: ReturnType<typeof setTimeout> | undefined;
let inflight = false;
let stopped = false;

function sessionName(metrics: SessionMetrics): string {
  if (metrics.prewarm) return t('agents.metrics.prewarm', { kind: metrics.prewarm.kind });
  return pacer?.sessions.value.get(sessionKey(metrics.session))?.name ?? t('agents.metrics.unnamed', { id: sessionId(metrics.session) });
}

const rows = computed<Row[]>(() => {
  const sessions = [...(snapshot.value?.sessions ?? [])].sort((a, b) => b.rss_bytes - a.rss_bytes);
  const largest = sessions[0]?.rss_bytes ?? 0;
  return sessions.map((s) => ({
    id: `${sessionKey(s.session)}:${s.pid}`,
    name: sessionName(s),
    procs: s.procs,
    rssBytes: s.rss_bytes,
    share: largest > 0 ? s.rss_bytes / largest : 0,
  }));
});

const totalBytes = computed(() => {
  const current = snapshot.value;
  if (!current) return 0;
  return current.daemon_rss_bytes + current.sessions.reduce((sum, s) => sum + s.rss_bytes, 0);
});

const systemBytes = computed(() => snapshot.value?.system_total_bytes ?? 0);

const totalText = computed(() => {
  const system = systemBytes.value;
  if (system <= 0) return formatBytes(totalBytes.value);
  const percent = Math.round((totalBytes.value / system) * 100);
  return t('agents.metrics.ofSystem', { used: formatBytes(totalBytes.value), total: formatBytes(system), percent });
});

function schedule(): void {
  clearTimeout(timer);
  if (stopped || document.hidden) return;
  timer = setTimeout(() => void poll(), POLL_INTERVAL_MS);
}

async function poll(): Promise<void> {
  if (inflight || stopped) return;
  inflight = true;
  try {
    const next = await getMetrics();
    if (stopped) return;
    snapshot.value = next;
    error.value = null;
  } catch (e) {
    if (!stopped) error.value = describeError(e);
  } finally {
    inflight = false;
    schedule();
  }
}

function onVisibilityChange(): void {
  if (document.hidden) {
    clearTimeout(timer);
    return;
  }
  if (inflight) return;
  clearTimeout(timer);
  void poll();
}

onMounted(() => {
  if (!pacer) return;
  document.addEventListener('visibilitychange', onVisibilityChange);
  if (!document.hidden) void poll();
});

onBeforeUnmount(() => {
  stopped = true;
  clearTimeout(timer);
  document.removeEventListener('visibilitychange', onVisibilityChange);
});
</script>

<template>
  <section class="metrics" :aria-label="t('agents.metrics.aria')" data-testid="metrics-panel">
    <p v-if="!pacer" class="note">{{ t('agents.metrics.unavailable') }}</p>
    <template v-else>
      <p v-if="error" class="error" role="alert" data-testid="metrics-error">{{ error }}</p>
      <p v-if="!snapshot && !error" class="note" role="status">{{ t('common.loading') }}</p>
      <template v-if="snapshot">
        <header class="summary">
          <span class="summary-label">{{ t('agents.metrics.total') }}</span>
          <strong class="summary-value" data-testid="metrics-total" :data-bytes="totalBytes">{{ totalText }}</strong>
          <span class="summary-detail">{{ t('agents.metrics.breakdown', { daemon: formatBytes(snapshot.daemon_rss_bytes), sessions: formatBytes(totalBytes - snapshot.daemon_rss_bytes) }) }}</span>
        </header>
        <p v-if="!rows.length" class="note">{{ t('agents.metrics.none') }}</p>
        <table v-else class="table">
          <thead>
            <tr>
              <th scope="col">{{ t('agents.metrics.session') }}</th>
              <th scope="col" class="num">{{ t('agents.metrics.processes') }}</th>
              <th scope="col" class="num">{{ t('agents.metrics.memory') }}</th>
              <th scope="col" class="bar-cell"><span class="visually-hidden">{{ t('agents.metrics.share') }}</span></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in rows" :key="row.id" data-testid="metrics-row" :data-rss-bytes="row.rssBytes">
              <td class="name" :title="row.name">{{ row.name }}</td>
              <td class="num">{{ row.procs }}</td>
              <td class="num">{{ formatBytes(row.rssBytes) }}</td>
              <td class="bar-cell">
                <div class="bar"><div class="bar-fill" :style="{ width: `${row.share * 100}%` }" /></div>
              </td>
            </tr>
          </tbody>
        </table>
      </template>
    </template>
  </section>
</template>

<style scoped>
.metrics {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  height: 100%;
  min-height: 0;
  padding: var(--vr-space-3);
  overflow-y: auto;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: var(--vr-color-surface);
}

.note {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.error {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
  border-radius: var(--vr-radius-md);
  background: var(--vr-color-surface-muted);
}

.summary {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--vr-space-1) var(--vr-space-3);
}

.summary-label,
.summary-detail {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.summary-value {
  font-size: var(--vr-font-size-lg);
  font-variant-numeric: tabular-nums;
}

.table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.table th,
.table td {
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  text-align: left;
  border-bottom: 1px solid var(--vr-color-border);
}

.table th {
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  color: var(--vr-color-text-muted);
}

.table .name {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.table .num {
  width: 96px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.table .bar-cell {
  width: 30%;
}

.bar {
  height: 6px;
  overflow: hidden;
  border-radius: var(--vr-radius-full);
  background: var(--vr-color-surface-muted);
}

.bar-fill {
  height: 100%;
  border-radius: var(--vr-radius-full);
  background: var(--vr-color-accent);
}

.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}
</style>
