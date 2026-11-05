<script setup lang="ts">
import { t } from '../i18n';
import { usePacer } from './pacer';
import LayoutA from './proto/LayoutA.vue';

localStorage.removeItem('veyra.agents.layout');

const pacer = usePacer();
</script>

<template>
  <div v-if="!pacer" class="unavailable" data-testid="pacer-banner" data-state="unavailable">{{ t('agents.unavailable') }}</div>
  <LayoutA v-else />
</template>

<style scoped>
.unavailable {
  margin: auto;
  color: var(--vr-color-text-muted);
}
</style>
