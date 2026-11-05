<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { Ellipsis } from 'lucide-vue-next';
import { VButton, VContextMenu, VInputText } from '@ui/kit';
import { archiveAgent, closeTerminal, deleteAgent, renameSession, restartAgent, type SessionNode, type SessionStatus } from './pacer';
import { useAction } from './useAction';
import MoveAgentDialog from './manage/MoveAgentDialog.vue';
import StatusDot from './StatusDot.vue';

const props = defineProps<{ session: SessionNode; active: boolean }>();
const emit = defineEmits<{ open: [] }>();

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

type Mode = 'idle' | 'renaming' | 'confirming-delete';
type MenuAction = { key: string; label: string; command: () => void };

const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const row = useTemplateRef<HTMLElement>('row');
const mode = shallowRef<Mode>('idle');
const draft = shallowRef('');
const action = useAction();
const moving = shallowRef(false);
const movableAgent = computed(() => ('Agent' in props.session.ref ? props.session.ref.Agent : null));

const menuItems = computed<MenuAction[]>(() => {
  const session = props.session.ref;
  if ('Terminal' in session)
    return [
      { key: 'rename', label: 'Переименовать', command: startRename },
      { key: 'close', label: 'Закрыть', command: () => void action.run(() => closeTerminal(session.Terminal)) },
    ];
  return [
    { key: 'rename', label: 'Переименовать', command: startRename },
    { key: 'restart', label: 'Перезапустить', command: () => void action.run(() => restartAgent(session.Agent)) },
    { key: 'move', label: 'Перенести…', command: () => (moving.value = true) },
    { key: 'archive', label: 'В архив', command: () => void action.run(() => archiveAgent(session.Agent)) },
    { key: 'delete', label: 'Удалить', command: () => (mode.value = 'confirming-delete') },
  ];
});

function showMenu(event: MouseEvent) {
  menu.value?.show(event);
}

async function startRename() {
  draft.value = props.session.name;
  action.clear();
  mode.value = 'renaming';
  await nextTick();
  const element = row.value?.querySelector('input.rename');
  if (element instanceof HTMLInputElement) element.select();
}

async function saveRename() {
  const name = draft.value.trim();
  if (!name || name === props.session.name) {
    mode.value = 'idle';
    return;
  }
  if (await action.run(() => renameSession(props.session.ref, name))) mode.value = 'idle';
}

function cancel() {
  mode.value = 'idle';
  action.clear();
}

async function confirmDelete() {
  const session = props.session.ref;
  if (!('Agent' in session)) return;
  if (await action.run(() => deleteAgent(session.Agent))) mode.value = 'idle';
}
</script>

<template>
  <div ref="row" class="session" :data-busy="action.busy.value">
    <VInputText
      v-if="mode === 'renaming'"
      v-model="draft"
      class="rename"
      size="small"
      aria-label="Новое имя сессии"
      data-testid="session-rename-input"
      :readonly="action.busy.value"
      @keydown.enter.prevent="saveRename"
      @keydown.esc.prevent="cancel"
    />
    <template v-else>
      <button
        class="row"
        data-testid="session-row"
        :data-session-id="session.id"
        :data-status="session.status"
        :data-unseen="session.unseen"
        :data-active="active"
        :title="`${session.name} — ${STATUS_LABELS[session.status]}`"
        @click="emit('open')"
        @contextmenu.prevent="showMenu"
      >
        <StatusDot :status="session.status" :unseen="session.unseen" />
        <span class="name">{{ session.name }}</span>
        <span class="label">{{ STATUS_LABELS[session.status] }}</span>
      </button>
      <button class="more" title="Действия" aria-label="Действия" aria-haspopup="menu" data-testid="session-menu" @click="showMenu"><Ellipsis :size="14" /></button>
    </template>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`session-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
    <div v-if="mode === 'confirming-delete'" class="confirm" role="group" aria-label="Подтверждение удаления" data-testid="session-delete-confirm">
      <span>Удалить «{{ session.name }}» насовсем?</span>
      <VButton label="Удалить" size="small" severity="danger" :loading="action.busy.value" data-testid="session-delete-yes" @click="confirmDelete" />
      <VButton label="Отмена" size="small" text data-testid="session-delete-no" @click="cancel" />
    </div>
    <MoveAgentDialog v-if="moving && movableAgent" :agent="movableAgent" @moved="moving = false" @close="moving = false" />
    <p v-if="action.error.value" class="row-error" role="alert" data-testid="session-error">{{ action.error.value }}</p>
  </div>
</template>

<style scoped>
.session {
  position: relative;
}

.session[data-busy='true'] .row {
  opacity: 0.6;
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

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
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

.session:hover .label,
.session:focus-within .label {
  visibility: hidden;
}

.row[data-status='needs_feedback'] .label {
  color: var(--vr-color-danger);
}

.row[data-unseen='true'] .label {
  color: var(--vr-color-success);
}

.more {
  position: absolute;
  top: 0;
  right: var(--vr-space-1);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: var(--vr-size-row);
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
  opacity: 0;
}

.session:hover .more,
.more:focus-visible {
  opacity: 1;
}

.more:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.rename {
  margin-left: var(--vr-space-4);
  width: calc(100% - var(--vr-space-4));
}

.confirm {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1) var(--vr-space-2);
  padding: var(--vr-space-1) var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
  font-size: var(--vr-font-size-xs);
}

.row-error {
  margin: 0;
  padding: 0 var(--vr-space-2) var(--vr-space-1) var(--vr-space-5);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-danger);
}

</style>
