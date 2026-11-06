<script setup lang="ts">
withDefaults(defineProps<{ rows?: number; height?: number }>(), { rows: 6, height: 20 });
</script>

<template>
  <div class="skeleton" aria-busy="true">
    <i v-for="row in rows" :key="row" class="line" :style="{ height: `${height}px`, width: `${100 - ((row * 17) % 40)}%` }" />
  </div>
</template>

<style scoped>
.skeleton {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
  padding: var(--vr-space-4);
}

.line {
  display: block;
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-surface-muted);
  animation: pulse 1.2s ease-in-out infinite;
}

@keyframes pulse {
  50% {
    opacity: 0.4;
  }
}
</style>
