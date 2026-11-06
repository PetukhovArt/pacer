<script setup lang="ts">
import { computed } from 'vue';
import { t } from '../../i18n';
import { useBerth } from '../berth';

const berth = useBerth();

const banner = computed(() => {
  const connection = berth?.connection.value;
  if (!connection) return null;
  switch (connection.state) {
    case 'connected':
      return null;
    case 'connecting':
      return t('agents.notices.connecting');
    case 'offline':
      return t('agents.notices.offline');
    case 'incompatible':
      return connection.message ?? t('agents.notices.incompatible');
    default: {
      const unknown: never = connection.state;
      return unknown;
    }
  }
});
</script>

<template>
  <template v-if="berth">
    <p v-if="banner" class="banner" role="status" data-testid="berth-banner" :data-state="berth.connection.value.state">{{ banner }}</p>
    <p v-if="berth.error.value" class="banner" role="alert" data-testid="berth-error">{{ berth.error.value }}</p>
  </template>
</template>

<style scoped>
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
</style>
