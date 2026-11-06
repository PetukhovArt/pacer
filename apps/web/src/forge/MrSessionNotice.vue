<script setup lang="ts">
import { LoaderCircle, TriangleAlert, X } from 'lucide-vue-next';
import { VButton } from '@ui/kit';
import { t } from '../i18n';
import RepoActions from '../workbench/RepoActions.vue';
import { useOpenInSession, type MrRun } from './openInSession';

defineProps<{ run: MrRun }>();

const { open, dismiss } = useOpenInSession();
</script>

<template>
  <div class="notice" :data-state="run.busy ? 'busy' : run.error ? 'error' : run.needsProject ? 'project' : 'warning'" role="status" data-testid="mr-session-notice">
    <div class="head">
      <LoaderCircle v-if="run.busy" :size="12" class="spin" />
      <TriangleAlert v-else :size="12" />
      <span class="label" :title="run.label">{{ run.label }}</span>
      <button v-if="!run.busy" class="close" type="button" :title="t('forge.openSession.dismiss')" :aria-label="t('forge.openSession.dismiss')" data-testid="mr-session-dismiss" @click="dismiss(run.id)"><X :size="12" /></button>
    </div>
    <p v-if="run.message" class="text" data-testid="mr-session-message">{{ run.message }}</p>
    <p v-if="run.error" class="text error" role="alert" data-testid="mr-session-error">{{ run.error }}</p>
    <p v-if="run.warning" class="text warning" data-testid="mr-session-warning">{{ run.warning }}</p>
    <div v-if="!run.busy && (run.error || run.needsProject)" class="actions">
      <RepoActions v-if="run.needsProject" :repo="run.needsProject" />
      <VButton :label="t('forge.openSession.retry')" size="small" text severity="secondary" data-testid="mr-session-retry" @click="open(run.item, run.refs)" />
    </div>
  </div>
</template>

<style scoped>
.notice {
  margin: 0 0 var(--vr-space-2);
  padding: var(--vr-space-2);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-muted);
  border-radius: var(--vr-radius-md);
}

.head {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
}

.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-weight: 500;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.close {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.close:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.text {
  margin: var(--vr-space-1) 0 0;
  overflow-wrap: anywhere;
  color: var(--vr-color-text-muted);
}

.error {
  color: var(--vr-color-danger);
}

.warning {
  color: var(--vr-color-conflict);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  margin-top: var(--vr-space-1);
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
