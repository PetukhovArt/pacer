<script setup lang="ts">
defineProps<{ status: string; category?: 'todo' | 'in_progress' | 'done' }>();
</script>

<template>
  <span class="chip" :data-category="category ?? 'todo'" :title="status">{{ status }}</span>
</template>

<style scoped>
.chip {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  padding: 1px var(--vr-space-2);
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-chip-bg);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
  line-height: 16px;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
  vertical-align: middle;
}

.chip[data-category='todo'] {
  color: var(--vr-color-text-muted);
}

.chip[data-category='in_progress'] {
  background: color-mix(in srgb, var(--vr-color-agent-running) 22%, transparent);
  color: var(--vr-color-agent-running);
}

.chip[data-category='done'] {
  background: color-mix(in srgb, var(--vr-color-success) 22%, transparent);
  color: var(--vr-color-success);
}
</style>
