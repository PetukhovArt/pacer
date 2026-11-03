<script setup lang="ts">
import { ArrowUp } from 'lucide-vue-next';

defineProps<{ placeholder: string; label: string; testid: string; sendTestid: string; sendLabel: string; disabled?: boolean }>();
const model = defineModel<string>({ default: '' });
const emit = defineEmits<{ submit: [] }>();

function send() {
  if (model.value.trim()) emit('submit');
}

function onKey(event: KeyboardEvent) {
  if (event.key !== 'Enter' || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  send();
}
</script>

<template>
  <form class="composer" @submit.prevent="send">
    <textarea
      v-model="model"
      rows="1"
      :placeholder="placeholder"
      :aria-label="label"
      :disabled="disabled"
      :data-testid="testid"
      @keydown="onKey"
    />
    <div class="toolbar">
      <div class="tools"><slot /></div>
      <button type="submit" class="send" :aria-label="sendLabel" :title="sendLabel" :disabled="disabled || !model.trim()" :data-testid="sendTestid">
        <ArrowUp :size="16" :stroke-width="2.5" />
      </button>
    </div>
  </form>
</template>

<style scoped>
.composer {
  display: flex;
  flex-direction: column;
  padding: var(--vr-space-3) var(--vr-space-4) var(--vr-space-2);
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-2xl);
}

.composer:focus-within {
  border-color: var(--vr-color-text-muted);
}

textarea {
  field-sizing: content;
  min-height: var(--vr-font-leading-message);
  max-height: 128px;
  padding: 0;
  font: inherit;
  color: inherit;
  background: none;
  border: 0;
  outline: none;
  resize: none;
}

textarea::placeholder {
  color: var(--vr-color-text-muted);
}

.toolbar {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  min-height: 40px;
  margin-top: var(--vr-space-2);
}

.tools {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  margin-left: calc(-1 * var(--vr-space-2));
}

.send {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-accent);
  border: 0;
  border-radius: 50%;
}

.send:disabled {
  background: var(--vr-color-text-muted);
  opacity: 0.6;
}
</style>
