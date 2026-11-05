<script setup lang="ts">
import 'dockview-vue/dist/styles/dockview.css';
import { onBeforeUnmount, shallowRef } from 'vue';
import { DockviewVue, themeLight, type DockviewApi, type DockviewReadyEvent } from 'dockview-vue';
import { ChevronRight, FileText, X } from 'lucide-vue-next';
import { t } from '../../i18n';
import MarkdownFile from '../../files/MarkdownFile.vue';
import { isSessionRef, usePacer } from '../pacer';
import SessionPanel from '../SessionPanel.vue';
import StatusDot from '../StatusDot.vue';
import MetricsPanel from '../manage/MetricsPanel.vue';
import { isFilePanel } from './markdownFiles';
import type { SessionPath } from './nav';
import { METRICS_PANEL, NAVIGATOR_PANEL } from './useAgentsDock';

const props = defineProps<{ activeKey: string | null; paths: ReadonlyMap<string, SessionPath>; groupLabels?: boolean }>();
const emit = defineEmits<{ ready: [event: DockviewReadyEvent]; 'open-file': [path: string, hash: string, from: string] }>();

type Host = { id: string; el: HTMLElement; panels: string[] };

const pacer = usePacer();
const hosts = shallowRef<Host[]>([]);
const subscriptions: { dispose(): void }[] = [];

function syncHosts(api: DockviewApi) {
  if (!props.groupLabels) return;
  hosts.value = api.groups.flatMap((group) => {
    const el = group.element.querySelector('.dv-pre-actions-container');
    return el instanceof HTMLElement ? [{ id: group.id, el, panels: group.panels.map((panel) => panel.id) }] : [];
  });
}

function onReady(event: DockviewReadyEvent) {
  const api = event.api;
  const sync = () => syncHosts(api);
  subscriptions.push(api.onDidAddGroup(sync), api.onDidRemoveGroup(sync), api.onDidAddPanel(sync), api.onDidRemovePanel(sync), api.onDidLayoutChange(sync));
  emit('ready', event);
  sync();
}

function labelOf(host: Host): SessionPath | undefined {
  for (const id of host.panels) {
    const path = props.paths.get(id);
    if (path) return path;
  }
  return undefined;
}

function tabTestId(id: string): string {
  if (id === METRICS_PANEL) return 'metrics-tab';
  if (id === NAVIGATOR_PANEL) return 'navigator-tab';
  if (isFilePanel(id)) return 'file-tab';
  return 'session-tab';
}

onBeforeUnmount(() => {
  for (const subscription of subscriptions) subscription.dispose();
});
</script>

<template>
  <div class="dock">
    <DockviewVue class="dockview" :theme="themeLight" @ready="onReady">
      <template #panel-session="{ params }">
        <SessionPanel v-if="isSessionRef(params.session)" class="vr-fade-in" :session="params.session" />
      </template>
      <template #panel-metrics>
        <MetricsPanel class="vr-fade-in" />
      </template>
      <template #panel-markdown="{ params, api }">
        <MarkdownFile v-if="typeof params.path === 'string'" class="vr-fade-in" :path="params.path" :hash="typeof params.hash === 'string' ? params.hash : ''" @open-file="(path, hash) => emit('open-file', path, hash, api.id)" />
      </template>
      <template #panel-navigator>
        <slot name="navigator" />
      </template>
      <template #defaultTab="{ api }">
        <span class="tab" :data-testid="tabTestId(api.id)" :data-session-id="pacer?.sessions.value.get(api.id)?.id" :data-active="api.id === activeKey">
          <FileText v-if="isFilePanel(api.id)" :size="12" />
          <StatusDot v-if="pacer?.sessions.value.has(api.id)" :status="pacer.sessions.value.get(api.id)?.status" :unseen="pacer.sessions.value.get(api.id)?.unseen" />
          <span class="name">{{ pacer?.sessions.value.get(api.id)?.name ?? api.title }}</span>
          <button class="close" :title="t('common.close')" :aria-label="t('common.close')" data-testid="session-tab-close" @pointerdown.stop @click.stop="api.close()"><X :size="12" /></button>
        </span>
      </template>
      <template #watermark>
        <p class="watermark">{{ t('agents.session.pick') }}</p>
      </template>
    </DockviewVue>
    <template v-for="host in hosts" :key="host.id">
      <Teleport v-if="labelOf(host)" :to="host.el">
        <span class="group-label" data-testid="group-label">
          <span class="group-project">{{ labelOf(host)?.project.name }}</span>
          <ChevronRight :size="11" />
          <span class="group-branch">{{ labelOf(host)?.worktree.branch }}</span>
        </span>
      </Teleport>
    </template>
  </div>
</template>

<style scoped>
.dock {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.dock :deep(.dockview) {
  height: 100%;
}

.dock :deep(.dockview-theme-light) {
  color-scheme: inherit;
  --dv-group-view-background-color: var(--vr-color-surface);
  --dv-tabs-and-actions-container-background-color: var(--vr-color-surface-muted);
  --dv-tabs-and-actions-container-font-size: var(--vr-font-size-xs);
  --dv-activegroup-visiblepanel-tab-background-color: var(--vr-color-surface);
  --dv-activegroup-hiddenpanel-tab-background-color: var(--vr-color-surface-muted);
  --dv-inactivegroup-visiblepanel-tab-background-color: var(--vr-color-surface);
  --dv-inactivegroup-hiddenpanel-tab-background-color: var(--vr-color-surface-muted);
  --dv-activegroup-visiblepanel-tab-color: var(--vr-color-text);
  --dv-activegroup-hiddenpanel-tab-color: var(--vr-color-text-muted);
  --dv-inactivegroup-visiblepanel-tab-color: var(--vr-color-text-muted);
  --dv-inactivegroup-hiddenpanel-tab-color: var(--vr-color-text-muted);
  --dv-tab-divider-color: var(--vr-color-border);
  --dv-separator-border: var(--vr-color-border);
  --dv-paneview-header-border-color: var(--vr-color-border);
  --dv-icon-hover-background-color: var(--vr-color-nav-hover);
  --dv-drag-over-background-color: var(--vr-color-nav-active);
  --dv-scrollbar-background-color: var(--vr-color-nav-active);
  --dv-tabs-container-scrollbar-color: var(--vr-color-nav-active);
}

.tab {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: 100%;
  padding: 0 var(--vr-space-1) 0 var(--vr-space-3);
}

.name {
  max-width: 200px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  visibility: hidden;
}

.tab:hover .close,
.tab[data-active='true'] .close {
  visibility: visible;
}

.close:hover {
  background: var(--vr-color-nav-hover);
}

.watermark {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  margin: 0;
  color: var(--vr-color-text-muted);
}

.group-label {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 100%;
  padding: 0 var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  border-right: 1px solid var(--vr-color-border);
}

.group-branch {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text);
}
</style>
