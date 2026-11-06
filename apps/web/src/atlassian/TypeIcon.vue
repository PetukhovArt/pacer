<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{ type: string }>();

const kind = computed(() => {
  const name = props.type.toLowerCase();
  if (/bug|ошиб|дефект/.test(name)) return 'bug';
  if (/epic|эпик/.test(name)) return 'epic';
  if (/sub|под/.test(name)) return 'subtask';
  if (/story|истори/.test(name)) return 'story';
  return 'task';
});

const glyph = computed(() => ({ bug: 'B', epic: 'E', subtask: 'S', story: 'U', task: 'T' })[kind.value]);
</script>

<template>
  <span class="type" :data-kind="kind" :title="type">{{ glyph }}</span>
</template>

<style scoped>
.type {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-agent-running);
  color: var(--vr-color-accent-contrast);
  font-size: var(--vr-font-size-2xs);
  font-weight: 700;
  line-height: 1;
  user-select: none;
}

.type[data-kind='bug'] {
  background: var(--vr-color-danger);
}

.type[data-kind='story'] {
  background: var(--vr-color-success);
}

.type[data-kind='epic'] {
  background: var(--vr-palette-avatar-6);
}

.type[data-kind='subtask'] {
  background: var(--vr-color-text-muted);
}
</style>
