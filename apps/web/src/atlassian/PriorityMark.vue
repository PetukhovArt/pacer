<script setup lang="ts">
import { computed } from 'vue';
import { priorityLevel } from '../priority';

const props = defineProps<{ priority: string | null }>();
const level = computed(() => priorityLevel(props.priority));
</script>

<template>
  <span v-if="priority" class="priority" :data-level="level" :title="priority"><i class="dot" />{{ priority }}</span>
  <span v-else class="none">-</span>
</template>

<style scoped>
.priority {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  max-width: 100%;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.dot {
  flex: none;
  width: 8px;
  height: 8px;
  border-radius: var(--vr-radius-full);
  background: var(--vr-color-text-muted);
}

.priority[data-level='blocker'] .dot,
.priority[data-level='high'] .dot {
  background: var(--vr-color-danger);
}

.priority[data-level='medium'] .dot {
  background: var(--vr-color-conflict);
}

.priority[data-level='low'] .dot {
  background: var(--vr-color-agent-running);
}

.none {
  color: var(--vr-color-text-muted);
}
</style>
