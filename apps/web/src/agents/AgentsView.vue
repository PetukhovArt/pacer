<script setup lang="ts">
import 'dockview-vue/dist/styles/dockview.css';
import { computed, onActivated, onBeforeUnmount, onDeactivated, shallowRef, watch } from 'vue';
import { DockviewVue, themeLight, type DockviewApi, type DockviewReadyEvent } from 'dockview-vue';
import { X } from 'lucide-vue-next';
import { isSessionRef, markSeen, usePacer, type SessionNode, type SessionStatus } from './pacer';
import SessionPanel from './SessionPanel.vue';

const STATUS_LABELS: Record<SessionStatus, string> = {
  fresh: 'новая',
  running: 'работает',
  needs_feedback: 'ждёт меня',
  finished: 'закончил',
  terminated: 'упал',
  disconnected: 'отключена',
  shell: 'терминал',
  closed: 'закрыт',
};

const pacer = usePacer();
const dock = shallowRef<DockviewApi | null>(null);
const activeKey = shallowRef<string | null>(null);
const visible = shallowRef(true);
let activeChange: { dispose(): void } | null = null;

const banner = computed(() => {
  const connection = pacer?.connection.value;
  if (!connection) return null;
  switch (connection.state) {
    case 'connected':
      return null;
    case 'connecting':
      return 'подключаюсь к pacer…';
    case 'offline':
      return 'демон pacer не запущен';
    case 'incompatible':
      return connection.message ?? 'несовместимая версия pacer';
    default: {
      const unknown: never = connection.state;
      return unknown;
    }
  }
});

const activeUnseenAgent = computed(() => {
  const session = activeKey.value ? pacer?.sessions.value.get(activeKey.value) : undefined;
  return visible.value && session?.unseen && 'Agent' in session.ref ? session.ref.Agent : null;
});

watch(activeUnseenAgent, (agent) => {
  if (agent) void markSeen(agent);
});

watch(
  () => pacer?.sessions.value,
  (sessions) => {
    const api = dock.value;
    if (!api || !sessions || pacer?.connection.value.state !== 'connected') return;
    for (const panel of api.panels) if (!sessions.has(panel.id)) panel.api.close();
  },
);

function onReady({ api }: DockviewReadyEvent) {
  dock.value = api;
  activeChange = api.onDidActivePanelChange(({ panel }) => {
    activeKey.value = panel?.id ?? null;
  });
}

function open(session: SessionNode) {
  const api = dock.value;
  if (!api) return;
  const existing = api.getPanel(session.key);
  if (existing) {
    existing.api.setActive();
    return;
  }
  api.addPanel({ id: session.key, component: 'session', title: session.name, params: { session: session.ref }, renderer: 'always' });
}

onActivated(() => {
  visible.value = true;
});
onDeactivated(() => {
  visible.value = false;
});
onBeforeUnmount(() => activeChange?.dispose());
</script>

<template>
  <div v-if="!pacer" class="unavailable" data-testid="pacer-banner" data-state="unavailable">Сессии pacer работают только в десктопном приложении Veyra</div>
  <div v-else class="agents">
    <aside class="tree" aria-label="Сессии pacer">
      <p v-if="banner" class="banner" role="status" data-testid="pacer-banner" :data-state="pacer.connection.value.state">{{ banner }}</p>
      <p v-if="pacer.error.value" class="banner" role="alert" data-testid="pacer-error">{{ pacer.error.value }}</p>
      <section v-for="workspace in pacer.tree.value" :key="workspace.id" class="workspace">
        <h2 class="workspace-name">{{ workspace.name }}</h2>
        <div v-for="project in workspace.projects" :key="project.id" class="project">
          <div class="project-name" :title="project.name">{{ project.name }}</div>
          <div v-for="worktree in project.worktrees" :key="worktree.id" class="worktree">
            <div class="branch" :title="worktree.branch">{{ worktree.branch }}</div>
            <button
              v-for="session in worktree.sessions"
              :key="session.key"
              class="row"
              data-testid="session-row"
              :data-session-id="session.id"
              :data-status="session.status"
              :data-unseen="session.unseen"
              :data-active="session.key === activeKey"
              :title="`${session.name} — ${STATUS_LABELS[session.status]}`"
              @click="open(session)"
            >
              <span class="dot" :data-status="session.status" :data-unseen="session.unseen" />
              <span class="name">{{ session.name }}</span>
              <span class="label">{{ STATUS_LABELS[session.status] }}</span>
            </button>
          </div>
        </div>
      </section>
      <p v-if="pacer.connection.value.state === 'connected' && !pacer.tree.value.length" class="empty">Нет сессий</p>
    </aside>
    <div class="dock">
      <DockviewVue class="dockview" :theme="themeLight" @ready="onReady">
        <template #panel-session="{ params }">
          <SessionPanel v-if="isSessionRef(params.session)" :session="params.session" />
        </template>
        <template #defaultTab="{ api }">
          <span class="tab" data-testid="session-tab" :data-session-id="pacer.sessions.value.get(api.id)?.id" :data-active="api.id === activeKey">
            <span class="dot" :data-status="pacer.sessions.value.get(api.id)?.status" :data-unseen="pacer.sessions.value.get(api.id)?.unseen" />
            <span class="name">{{ pacer.sessions.value.get(api.id)?.name ?? api.title }}</span>
            <button class="close" title="Закрыть" aria-label="Закрыть" data-testid="session-tab-close" @pointerdown.stop @click.stop="api.close()"><X :size="12" /></button>
          </span>
        </template>
        <template #watermark>
          <p class="watermark">Выберите сессию слева</p>
        </template>
      </DockviewVue>
    </div>
  </div>
</template>

<style scoped>
.unavailable {
  margin: auto;
  color: var(--vr-color-text-muted);
}

.agents {
  display: grid;
  grid-template-columns: 280px minmax(0, 1fr);
  flex: 1;
  min-height: 0;
}

.tree {
  min-height: 0;
  overflow-y: auto;
  padding: var(--vr-space-2);
  border-right: 1px solid var(--vr-color-border);
}

.banner {
  margin: 0 0 var(--vr-space-2);
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  border-radius: var(--vr-radius-md);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
}

.banner[role='alert'],
.banner[data-state='offline'],
.banner[data-state='incompatible'] {
  color: var(--vr-color-danger);
}

.workspace + .workspace {
  margin-top: var(--vr-space-3);
}

.workspace-name {
  margin: 0;
  padding: 0 var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  font-weight: 500;
  line-height: var(--vr-size-row);
  color: var(--vr-color-text-muted);
}

.project-name,
.branch,
.name {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.project-name {
  padding: 0 var(--vr-space-2);
  font-weight: 600;
  line-height: var(--vr-size-row);
}

.branch {
  padding: 0 var(--vr-space-2) 0 var(--vr-space-4);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  line-height: var(--vr-size-row);
  color: var(--vr-color-text-muted);
}

.row {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  width: 100%;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2) 0 var(--vr-space-5);
  font: inherit;
  text-align: left;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.row:hover {
  background: var(--vr-color-nav-hover);
}

.row[data-active='true'] {
  background: var(--vr-color-nav-active);
}

.row .name {
  flex: 1;
  min-width: 0;
}

.row[data-unseen='true'] .name,
.row[data-unseen='true'] .label {
  font-weight: 600;
}

.label {
  flex: none;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.row[data-status='needs_feedback'] .label {
  color: var(--vr-color-danger);
}

.row[data-unseen='true'] .label {
  color: var(--vr-color-success);
}

.dot {
  flex: none;
  box-sizing: border-box;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--vr-color-border);
}

.dot[data-status='running'] {
  background: var(--vr-color-conflict);
}

.dot[data-status='needs_feedback'] {
  background: var(--vr-color-danger);
}

.dot[data-status='finished'] {
  background: var(--vr-color-success);
}

.dot[data-unseen='true'] {
  box-shadow: 0 0 0 3px var(--vr-color-chip-bg);
  outline: 1px solid var(--vr-color-success);
  outline-offset: 1px;
}

.dot[data-status='terminated'] {
  background: none;
  border: 2px solid var(--vr-color-danger);
}

.dot[data-status='disconnected'],
.dot[data-status='closed'] {
  background: none;
  border: 2px solid var(--vr-color-border);
}

.dot[data-status='shell'] {
  background: var(--vr-color-text-muted);
}

.empty {
  margin: 0;
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.dock {
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

.tab .name {
  max-width: 200px;
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
</style>
