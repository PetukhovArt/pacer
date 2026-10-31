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
  gap: 4px;
}

.priority,
.label {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 0 6px;
  font-size: 11px;
  line-height: 18px;
  border-radius: 9px;
  border: 1px solid var(--vr-color-border);
}

.priority img {
  width: 12px;
  height: 12px;
}

.priority[data-level='blocker'] {
  color: #fff;
  background: #c62828;
  border-color: #c62828;
}

.priority[data-level='high'] {
  color: #c62828;
  border-color: #c62828;
}

.priority[data-level='medium'] {
  color: #ef6c00;
  border-color: #ef6c00;
}

.priority[data-level='low'],
.priority[data-level='trivial'] {
  color: var(--vr-color-text-muted);
}

.label {
  background: var(--vr-color-surface-muted);
}
</style>
