<script setup lang="ts">
import { VButton } from '@ui/kit';

defineProps<{ message: string }>();
defineEmits<{ retry: [] }>();
</script>

<template>
  <div class="error" role="alert">
    <span class="text">{{ message }}</span>
    <VButton label="Повторить" size="small" severity="secondary" @click="$emit('retry')" />
  </div>
</template>

<style scoped>
.error {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  margin: var(--vr-space-4);
  padding: var(--vr-space-3) var(--vr-space-4);
  border: 1px solid var(--vr-color-danger);
  border-radius: var(--vr-radius-md);
  background: color-mix(in srgb, var(--vr-color-danger) 10%, transparent);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
}

.text {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
</style>
