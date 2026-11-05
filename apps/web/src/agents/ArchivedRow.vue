<script setup lang="ts">
import { shallowRef } from 'vue';
import { VButton } from '@ui/kit';
import { deleteAgent, unarchiveAgent, type SessionNode } from './pacer';
import { useAction } from './useAction';

const props = defineProps<{ session: SessionNode }>();

const confirming = shallowRef(false);
const action = useAction();

function restore() {
  void action.run(() => unarchiveAgent(props.session.id));
}

async function confirmDelete() {
  if (await action.run(() => deleteAgent(props.session.id))) confirming.value = false;
}
</script>

<template>
  <div class="archived" data-testid="archived-row" :data-session-id="session.id" :data-busy="action.busy.value">
    <div class="line">
      <span class="name" :title="session.name">{{ session.name }}</span>
      <template v-if="!confirming">
        <VButton label="Восстановить" size="small" text :loading="action.busy.value" data-testid="archived-restore" @click="restore" />
        <VButton label="Удалить" size="small" text severity="danger" data-testid="archived-delete" @click="confirming = true" />
      </template>
    </div>
    <div v-if="confirming" class="confirm" role="group" aria-label="Подтверждение удаления" data-testid="session-delete-confirm">
      <span>Удалить «{{ session.name }}» насовсем?</span>
      <VButton label="Удалить" size="small" severity="danger" :loading="action.busy.value" data-testid="session-delete-yes" @click="confirmDelete" />
      <VButton label="Отмена" size="small" text data-testid="session-delete-no" @click="confirming = false" />
    </div>
    <p v-if="action.error.value" class="row-error" role="alert" data-testid="session-error">{{ action.error.value }}</p>
  </div>
</template>

<style scoped>
.line {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-height: var(--vr-size-row);
  padding: 0 var(--vr-space-1) 0 var(--vr-space-6);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--vr-color-text-muted);
}

.archived[data-busy='true'] .name {
  opacity: 0.6;
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
