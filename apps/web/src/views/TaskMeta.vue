<script setup lang="ts">
import type { Task } from '@veyra/protocol';
import { priorityLevel } from '../priority';

defineProps<{ task: Task }>();
</script>

<template>
  <span v-if="task.priority || task.labels?.length" class="task-meta">
    <span v-if="task.priority" class="priority" :data-level="priorityLevel(task.priority)" data-testid="task-priority">
      <img v-if="task.priorityIconUrl" :src="task.priorityIconUrl" alt="" />
      {{ task.priority }}
    </span>
    <span v-for="label in task.labels" :key="label" class="label" data-testid="task-label">{{ label }}</span>
  </span>
</template>

<style scoped>
.task-meta {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
}

.priority,
.label {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 0 6px;
  font-size: var(--vr-font-size-2xs);
  font-weight: 500;
  line-height: 18px;
  border-radius: var(--vr-radius-full);
  color: var(--vr-color-text-muted);
  background: var(--vr-color-surface-muted);
}

.priority img {
  width: 12px;
  height: 12px;
}

.priority[data-level='blocker'] {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-danger);
}

.priority[data-level='high'] {
  color: var(--vr-color-danger);
}

.priority[data-level='medium'] {
  color: var(--vr-color-conflict);
}
</style>
