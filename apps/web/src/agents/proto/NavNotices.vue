<script setup lang="ts">
import { computed } from 'vue';
import { usePacer } from '../pacer';

const pacer = usePacer();

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
</script>

<template>
  <template v-if="pacer">
    <p v-if="banner" class="banner" role="status" data-testid="pacer-banner" :data-state="pacer.connection.value.state">{{ banner }}</p>
    <p v-if="pacer.error.value" class="banner" role="alert" data-testid="pacer-error">{{ pacer.error.value }}</p>
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
