<script setup lang="ts">
import { ChevronLeft } from 'lucide-vue-next';
import { useI18n } from '../i18n';

defineProps<{ title?: string }>();
const emit = defineEmits<{ back: [] }>();

const { t } = useI18n();
</script>

<template>
  <div class="bar">
    <button class="back" data-testid="mobile-back" @click="emit('back')"><ChevronLeft :size="18" />{{ t('mobile.back') }}</button>
    <span v-if="title" class="title">{{ title }}</span>
  </div>
</template>

<style scoped>
.bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: 2.75rem;
  padding: 0 var(--vr-space-2);
  border-bottom: 1px solid var(--vr-color-border);
}

.back {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 2.25rem;
  padding: 0 var(--vr-space-2);
  font: inherit;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.title {
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
