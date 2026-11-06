<script setup lang="ts">
import { computed } from 'vue';
import { VButton, VDialog } from '@ui/kit';
import { t } from '../i18n';
import { fileNameOf } from '../agents/proto/markdownFiles';
import { unsavedPrompt } from './documents';

const names = computed(() => (unsavedPrompt.value?.paths ?? []).map(fileNameOf).join(', '));

function onVisible(visible: boolean) {
  if (!visible) unsavedPrompt.value?.settle('cancel');
}
</script>

<template>
  <VDialog :visible="unsavedPrompt !== null" modal :header="t('files.editor.unsaved.title')" :style="{ width: '440px' }" :draggable="false" @update:visible="onVisible">
    <p class="message" data-testid="unsaved-message">{{ t('files.editor.unsaved.message', { names }) }}</p>
    <div class="actions">
      <VButton :label="t('common.cancel')" size="small" text data-testid="unsaved-cancel" @click="unsavedPrompt?.settle('cancel')" />
      <VButton :label="t('files.editor.unsaved.discard')" size="small" severity="danger" text data-testid="unsaved-discard" @click="unsavedPrompt?.settle('discard')" />
      <VButton :label="t('common.save')" size="small" data-testid="unsaved-save" @click="unsavedPrompt?.settle('save')" />
    </div>
  </VDialog>
</template>

<style scoped>
.message {
  margin: 0 0 var(--vr-space-4);
  overflow-wrap: anywhere;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}
</style>
