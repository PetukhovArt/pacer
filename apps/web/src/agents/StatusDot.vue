<script setup lang="ts">
import type { SessionStatus } from './berth';

defineProps<{ status: SessionStatus | undefined; unseen: boolean | undefined }>();
</script>

<template>
  <span class="dot" :data-status="status" :data-unseen="unseen" />
</template>

<style scoped>
.dot {
  flex: none;
  box-sizing: border-box;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--vr-color-border);
}

.dot[data-status='running'] {
  background: var(--vr-color-conflict);
}

.dot[data-status='needs_feedback'] {
  background: var(--vr-color-danger);
}

.dot[data-status='finished'] {
  background: var(--vr-color-success);
}

.dot[data-unseen='true'] {
  box-shadow: 0 0 0 3px var(--vr-color-chip-bg);
  outline: 1px solid var(--vr-color-success);
  outline-offset: 1px;
}

.dot[data-status='terminated'] {
  background: none;
  border: 2px solid var(--vr-color-danger);
}

.dot[data-status='disconnected'],
.dot[data-status='closed'] {
  background: none;
  border: 2px solid var(--vr-color-border);
}

.dot[data-status='shell'] {
  background: var(--vr-color-text-muted);
}
</style>
