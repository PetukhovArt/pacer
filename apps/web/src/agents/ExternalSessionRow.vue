<script setup lang="ts">
import { computed, useTemplateRef } from 'vue';
import { SquareTerminal } from 'lucide-vue-next';
import { VContextMenu } from '@ui/kit';
import { copyText } from '../atlassian/util';
import { useI18n } from '../i18n';
import { externalDot, externalOrigin, externalStatusLabel, resumeCommand, type ExternalSession } from './external';
import HarnessIcon from './HarnessIcon.vue';
import StatusDot from './StatusDot.vue';

const props = defineProps<{ session: ExternalSession; active: boolean }>();
const emit = defineEmits<{ open: [] }>();

const { t, formatRelative, formatDate } = useI18n();
const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');

const origin = computed(() => externalOrigin(props.session));
const tooltip = computed(() =>
  [props.session.title, `${externalStatusLabel(props.session)} · ${formatDate(props.session.updated_at)}`, props.session.branch ? `${props.session.cwd} · ${props.session.branch}` : props.session.cwd, origin.value].join('\n'),
);
const menuItems = computed(() => [
  { key: 'open', label: t('workbench.external.open'), command: () => emit('open') },
  { key: 'copy', label: t('workbench.external.copyResume'), command: () => void copyText(resumeCommand(props.session)) },
]);
</script>

<template>
  <div class="external">
    <button
      class="row"
      data-testid="external-session-row"
      :data-session-id="session.id"
      :data-harness="session.harness"
      :data-status="session.status"
      :data-running="session.running"
      :data-active="active"
      :title="tooltip"
      @click="emit('open')"
      @contextmenu.prevent="menu?.show($event)"
    >
      <StatusDot :status="externalDot(session)" :unseen="false" />
      <HarnessIcon :harness="session.harness" />
      <span class="name">{{ session.title }}</span>
      <span class="marker" :title="origin" :aria-label="origin" role="img" data-testid="external-marker"><SquareTerminal :size="11" /></span>
      <span class="label">{{ session.running ? externalStatusLabel(session) : formatRelative(session.updated_at) }}</span>
    </button>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`external-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
  </div>
</template>

<style scoped>
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

.row[data-running='false'] .name {
  color: var(--vr-color-text-muted);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.marker {
  display: inline-flex;
  flex: none;
  color: var(--vr-color-text-muted);
  opacity: 0.7;
}

.label {
  flex: none;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.row[data-running='true'][data-status='waiting'] .label {
  color: var(--vr-color-danger);
}
</style>
