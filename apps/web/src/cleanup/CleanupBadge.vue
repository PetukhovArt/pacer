<script setup lang="ts">
import { computed } from 'vue';
import { t } from '../i18n';
import { badges, openCleanup } from './state';

const props = defineProps<{ worktreeId: string }>();

const kind = computed(() => badges.value.get(props.worktreeId) ?? null);
const label = computed(() => (kind.value ? t(`cleanup.badge.${kind.value}`) : ''));
</script>

<template>
  <button v-if="kind" class="badge" :data-kind="kind" :title="t('cleanup.badgeTitle', { reason: label })" :aria-label="t('cleanup.badgeTitle', { reason: label })" data-testid="worktree-cleanup-badge" @click.stop="openCleanup">{{ label }}</button>
</template>

<style scoped>
.badge {
  flex: none;
  padding: 0 var(--vr-space-1);
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  line-height: 16px;
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border: none;
  border-radius: var(--vr-radius-full);
  cursor: pointer;
}

.badge[data-kind='merged'] {
  color: var(--vr-color-success);
}

.badge[data-kind='closed'],
.badge[data-kind='gone'] {
  color: var(--vr-color-conflict);
}

.badge:hover {
  color: var(--vr-color-text);
}
</style>
