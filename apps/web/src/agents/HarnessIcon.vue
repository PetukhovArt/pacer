<script setup lang="ts">
import { t } from '../i18n';
import type { Harness } from './external';

withDefaults(defineProps<{ harness: Harness; size?: number }>(), { size: 12 });
</script>

<template>
  <svg class="harness" :data-harness="harness" :width="size" :height="size" viewBox="0 0 24 24" role="img" :aria-label="t(`workbench.external.harness.${harness}`)">
    <title>{{ t(`workbench.external.harness.${harness}`) }}</title>
    <g v-if="harness === 'claude'" stroke="currentColor" stroke-width="2.6" stroke-linecap="round">
      <path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" />
    </g>
    <g v-else fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round">
      <path d="M12 2.8 20 7.4v9.2L12 21.2 4 16.6V7.4Z" />
      <path d="m9 9.5 2.5 2.5L9 14.5M13 14.5h3" stroke-linecap="round" />
    </g>
  </svg>
</template>

<style scoped>
.harness {
  flex: none;
}

.harness[data-harness='claude'] {
  color: var(--vr-color-conflict);
}

.harness[data-harness='codex'] {
  color: var(--vr-color-text-muted);
}
</style>
