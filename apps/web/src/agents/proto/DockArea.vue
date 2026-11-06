<script setup lang="ts">
import 'dockview-vue/dist/styles/dockview.css';
import { onBeforeUnmount, shallowRef } from 'vue';
import { DockviewVue, themeLight, type DockviewApi, type DockviewReadyEvent } from 'dockview-vue';
import { ChevronRight, FileText, GitPullRequest, ListTodo, Network, Ticket, X } from 'lucide-vue-next';
import JiraIssueView from '../../atlassian/JiraIssueView.vue';
import ArchitectureView from '../../forge/ArchitectureView.vue';
import ChangeRequestView from '../../forge/ChangeRequestView.vue';
import { t } from '../../i18n';
import MarkdownFile from '../../files/MarkdownFile.vue';
import TaskView from '../../views/TaskView.vue';
import ConversationView from '../../workbench/ConversationView.vue';
import { METRICS_PANEL, useDock } from '../../workbench/dock';
import { externalDot, isExternalPanel, useExternalSessions, type Harness } from '../external';
import HarnessIcon from '../HarnessIcon.vue';
import { isSessionRef, usePacer } from '../pacer';
import SessionPanel from '../SessionPanel.vue';
import StatusDot from '../StatusDot.vue';
import MetricsPanel from '../manage/MetricsPanel.vue';
import { isDirtyPath, settleDirty } from '../../files/documents';
import { FILE_PANEL_PREFIX, isFilePanel } from './markdownFiles';
import type { SessionPath } from './nav';

const props = defineProps<{ activeKey: string | null; paths: ReadonlyMap<string, SessionPath>; groupLabels?: boolean }>();
const emit = defineEmits<{ ready: [event: DockviewReadyEvent]; 'open-file': [path: string, hash: string, from: string] }>();

type Host = { id: string; el: HTMLElement; panels: string[] };

const pacer = usePacer();
const external = useExternalSessions();
const dock = useDock();
const isHarness = (value: unknown): value is Harness => value === 'claude' || value === 'codex';
const tabHarness = (id: string): Harness => (id.startsWith('ext:codex:') ? 'codex' : 'claude');
const tabDot = (id: string) => {
  const session = external?.anyByKey.value.get(id);
  return session ? externalDot(session) : 'closed';
};
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

function kindOf(id: string): 'metrics' | 'file' | 'review' | 'architecture' | 'issue' | 'task' | 'conversation' | 'session' {
  if (id === METRICS_PANEL) return 'metrics';
  if (id.startsWith('arch:')) return 'architecture';
  if (isExternalPanel(id)) return 'conversation';
  if (isFilePanel(id)) return 'file';
  if (id.startsWith('mr:')) return 'review';
  if (id.startsWith('jira:')) return 'issue';
  if (id.startsWith('task:')) return 'task';
  return 'session';
}

async function closeTab(api: { id: string; close: () => void }) {
  if (isFilePanel(api.id) && !(await settleDirty([api.id.slice(FILE_PANEL_PREFIX.length)]))) return;
  api.close();
}

const str = (value: unknown) => (typeof value === 'string' ? value : '');

onBeforeUnmount(() => {
  for (const subscription of subscriptions) subscription.dispose();
});
</script>

<template>
  <div class="dock">
    <DockviewVue class="dockview" :theme="themeLight" @ready="onReady">
      <template #panel-session="{ params, api }">
        <SessionPanel v-if="isSessionRef(params.session)" class="vr-fade-in" :session="params.session" :panel="api" />
      </template>
      <template #panel-review="{ params }">
        <ChangeRequestView v-if="typeof params.connector === 'string' && typeof params.project === 'string' && typeof params.number === 'number'" class="vr-fade-in" :connector="params.connector" :project="params.project" :number="params.number" />
      </template>
      <template #panel-architecture="{ params }">
        <ArchitectureView
          v-if="str(params.connector) && str(params.project) && typeof params.number === 'number' && str(params.repo)"
          class="vr-fade-in"
          :connector="str(params.connector)"
          :project="str(params.project)"
          :number="params.number"
          :repo="str(params.repo)"
          :target="str(params.target)"
        />
      </template>
      <template #panel-issue="{ params }">
        <JiraIssueView v-if="typeof params.connector === 'string' && typeof params.key === 'string'" class="vr-fade-in" :connector="params.connector" :issue-key="params.key" @open="(ref) => dock.openIssue(ref.connector, ref.key)" />
      </template>
      <template #panel-task="{ params }">
        <TaskView v-if="str(params.id)" class="vr-fade-in" :task-id="str(params.id)" embedded />
      </template>
      <template #panel-conversation="{ params }">
        <ConversationView v-if="isHarness(params.harness) && typeof params.id === 'string'" class="vr-fade-in" :harness="params.harness" :session-id="params.id" />
      </template>
      <template #panel-metrics>
        <MetricsPanel class="vr-fade-in" />
      </template>
      <template #panel-markdown="{ params, api }">
        <MarkdownFile v-if="typeof params.path === 'string'" class="vr-fade-in" :path="params.path" :hash="typeof params.hash === 'string' ? params.hash : ''" @open-file="(path, hash) => emit('open-file', path, hash, api.id)" />
      </template>
      <template #defaultTab="{ api }">
        <span class="tab" :data-testid="`${kindOf(api.id)}-tab`" :data-panel-id="api.id" :data-session-id="pacer?.sessions.value.get(api.id)?.id" :data-active="api.id === activeKey">
          <FileText v-if="kindOf(api.id) === 'file'" :size="12" />
          <GitPullRequest v-else-if="kindOf(api.id) === 'review'" :size="12" />
          <Network v-else-if="kindOf(api.id) === 'architecture'" :size="12" />
          <Ticket v-else-if="kindOf(api.id) === 'issue'" :size="12" />
          <ListTodo v-else-if="kindOf(api.id) === 'task'" :size="12" />
          <template v-else-if="kindOf(api.id) === 'conversation'">
            <StatusDot :status="tabDot(api.id)" :unseen="false" />
            <HarnessIcon :harness="tabHarness(api.id)" />
          </template>
          <StatusDot v-if="pacer?.sessions.value.has(api.id)" :status="pacer.sessions.value.get(api.id)?.status" :unseen="pacer.sessions.value.get(api.id)?.unseen" />
          <span class="name">{{ pacer?.sessions.value.get(api.id)?.name ?? external?.anyByKey.value.get(api.id)?.title ?? api.title }}</span>
          <span v-if="isFilePanel(api.id) && isDirtyPath(api.id.slice(FILE_PANEL_PREFIX.length))" class="dirty" :title="t('files.editor.dirty')" data-testid="file-tab-dirty" />
          <button class="close" :title="t('common.close')" :aria-label="t('common.close')" data-testid="session-tab-close" @pointerdown.stop @click.stop="closeTab(api)"><X :size="12" /></button>
        </span>
      </template>
      <template #watermark>
        <p class="watermark">{{ t('workbench.pick') }}</p>
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

.dirty {
  flex: none;
  width: 7px;
  height: 7px;
  background: var(--vr-color-accent);
  border-radius: var(--vr-radius-full);
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
