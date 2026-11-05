<script setup lang="ts">
import { onBeforeUnmount, onMounted, shallowRef } from 'vue';
import { Copy, RefreshCw, ShieldAlert, Smartphone } from 'lucide-vue-next';
import { VButton, VMessage } from '@ui/kit';
import { invoke, isDesktop } from '../bridge';
import { describeError } from '../connectors/api';
import { useI18n } from '../i18n';

type Problem = { kind: 'tailscale_missing' } | { kind: 'bind_failed'; message: string } | { kind: 'token_store'; message: string };
type ClientInfo = { id: number; address: string; agent: string; connected_at: number };
type RemoteStatus = {
  enabled: boolean;
  running: boolean;
  problem: Problem | null;
  urls: string[];
  pairing_url: string | null;
  qr_svg: string | null;
  clients: ClientInfo[];
};

const POLL_MS = 2000;
const COPIED_MS = 1500;

const { t } = useI18n();
const status = shallowRef<RemoteStatus | null>(null);
const failure = shallowRef<string | null>(null);
const busy = shallowRef(false);
const copied = shallowRef(false);
let timer: ReturnType<typeof setInterval> | undefined;

async function load() {
  try {
    status.value = await invoke<RemoteStatus>('remote_status');
    failure.value = null;
  } catch (e) {
    failure.value = describeError(e);
  }
}

async function regenerate() {
  busy.value = true;
  try {
    status.value = await invoke<RemoteStatus>('remote_regenerate_token');
    failure.value = null;
  } catch (e) {
    failure.value = describeError(e);
  } finally {
    busy.value = false;
  }
}

async function copy(url: string) {
  await navigator.clipboard.writeText(url);
  copied.value = true;
  setTimeout(() => (copied.value = false), COPIED_MS);
}

function problemText(problem: Problem): string {
  if (problem.kind === 'tailscale_missing') return t('remote.panel.problem.tailscale_missing');
  return t(`remote.panel.problem.${problem.kind}`, { message: problem.message });
}

const since = (ms: number) => t('remote.panel.connectedAt', { time: new Date(ms).toLocaleTimeString() });

onMounted(() => {
  if (!isDesktop) return;
  void load();
  timer = setInterval(() => void load(), POLL_MS);
});

onBeforeUnmount(() => clearInterval(timer));
</script>

<template>
  <div class="remote" data-testid="remote-panel">
    <VMessage severity="warn" size="small" data-testid="remote-warning">
      <strong class="warning-title"><ShieldAlert :size="14" />{{ t('remote.panel.warningTitle') }}</strong>
      {{ t('remote.panel.warning') }}
    </VMessage>
    <p v-if="!isDesktop" class="muted">{{ t('remote.panel.desktopOnly') }}</p>
    <template v-else-if="status">
      <VMessage v-if="status.problem" severity="error" size="small" data-testid="remote-problem">{{ problemText(status.problem) }}</VMessage>
      <VMessage v-if="failure" severity="error" size="small">{{ failure }}</VMessage>
      <p v-if="!status.enabled" class="muted" data-testid="remote-off">{{ t('remote.panel.off') }}</p>
      <p v-else-if="!status.running && !status.problem" class="muted">{{ t('remote.panel.starting') }}</p>
      <template v-if="status.running">
        <p class="state" data-testid="remote-running">{{ t('remote.panel.running') }}</p>
        <div class="pairing">
          <div v-if="status.qr_svg" class="qr" role="img" :aria-label="t('remote.panel.qrLabel')" data-testid="remote-qr" v-html="status.qr_svg" />
          <div class="details">
            <h3>{{ t('remote.panel.addresses') }}</h3>
            <ul class="urls">
              <li v-for="url in status.urls" :key="url"><code>{{ url }}</code></li>
            </ul>
            <template v-if="status.pairing_url">
              <h3>{{ t('remote.panel.pairing') }}</h3>
              <code class="link" data-testid="remote-pairing-url">{{ status.pairing_url }}</code>
              <p class="muted">{{ t('remote.panel.pairingHint') }}</p>
              <div class="actions">
                <VButton size="small" severity="secondary" :label="copied ? t('remote.panel.copied') : t('remote.panel.copy')" data-testid="remote-copy" @click="copy(status.pairing_url)">
                  <template #icon><Copy :size="14" /></template>
                </VButton>
                <VButton size="small" severity="danger" :label="t('remote.panel.regenerate')" :title="t('remote.panel.regenerateHint')" :disabled="busy" data-testid="remote-regenerate" @click="regenerate">
                  <template #icon><RefreshCw :size="14" /></template>
                </VButton>
              </div>
            </template>
          </div>
        </div>
        <h3>{{ t('remote.panel.clients') }}</h3>
        <ul v-if="status.clients.length" class="clients" data-testid="remote-clients">
          <li v-for="client in status.clients" :key="client.id">
            <Smartphone :size="14" />
            <code>{{ client.address }}</code>
            <span class="muted agent" :title="client.agent">{{ client.agent }}</span>
            <span class="muted">{{ since(client.connected_at) }}</span>
          </li>
        </ul>
        <p v-else class="muted">{{ t('remote.panel.noClients') }}</p>
      </template>
    </template>
  </div>
</template>

<style scoped>
.remote {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  padding: var(--vr-space-4) 0;
}

.warning-title {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  margin-right: var(--vr-space-1);
}

.muted {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.state {
  margin: 0;
  font-weight: 600;
}

.pairing {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-4);
  align-items: flex-start;
}

.qr {
  flex: none;
  width: 220px;
  padding: var(--vr-space-2);
  border-radius: var(--vr-radius-md);
}

.qr :deep(svg) {
  display: block;
  width: 100%;
  height: auto;
}

.details {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--vr-space-2);
  min-width: 16rem;
}

h3 {
  margin: var(--vr-space-2) 0 0;
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-muted);
  text-transform: uppercase;
}

.urls,
.clients {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}

.clients li {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.agent {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.link {
  word-break: break-all;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
}
</style>
