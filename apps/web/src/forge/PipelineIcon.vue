<script setup lang="ts">
import { Ban, CircleCheck, CircleDashed, CircleX, Clock, LoaderCircle } from 'lucide-vue-next';
import type { PipelineStatus } from '../connectors/api';

withDefaults(defineProps<{ status: PipelineStatus; size?: number }>(), { size: 14 });
</script>

<template>
  <span class="pipeline-icon" :data-status="status" :data-testid="`pipeline-${status}`">
    <CircleCheck v-if="status === 'success'" :size="size" />
    <CircleX v-else-if="status === 'failed'" :size="size" />
    <LoaderCircle v-else-if="status === 'running'" :size="size" class="spin" />
    <Clock v-else-if="status === 'pending'" :size="size" />
    <Ban v-else-if="status === 'canceled'" :size="size" />
    <CircleDashed v-else :size="size" />
  </span>
</template>

<style scoped>
.pipeline-icon {
  display: inline-flex;
  flex: none;
  color: var(--vr-color-text-muted);
}

.pipeline-icon[data-status='success'] {
  color: var(--vr-color-success);
}

.pipeline-icon[data-status='failed'] {
  color: var(--vr-color-danger);
}

.pipeline-icon[data-status='running'] {
  color: var(--vr-color-agent-running);
}

.pipeline-icon[data-status='pending'] {
  color: var(--vr-color-conflict);
}

.spin {
  animation: spin 1.2s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
