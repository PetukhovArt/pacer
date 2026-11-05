<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { VButton, VMessage, VTextarea } from '@ui/kit';
import { useSettings } from './store';

const EMPTY = '{}\n';

const { text, saveText } = useSettings();
const draft = shallowRef(text.value.trim() ? text.value : EMPTY);
const error = shallowRef<string | null>(null);
const saved = shallowRef(false);
const dirty = computed(() => draft.value !== (text.value.trim() ? text.value : EMPTY));

watch(text, (next, previous) => {
  if (draft.value === (previous.trim() ? previous : EMPTY)) draft.value = next.trim() ? next : EMPTY;
});

function onInput(value: string | undefined) {
  draft.value = value ?? '';
  error.value = null;
  saved.value = false;
}

function save() {
  const problem = saveText(draft.value);
  error.value = problem;
  saved.value = problem === null;
}

function onKey(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.code === 'KeyS') {
    event.preventDefault();
    save();
  }
}
</script>

<template>
  <div class="editor" @keydown="onKey">
    <div class="bar">
      <span class="path">~/.veyra/settings.json</span>
      <span v-if="dirty" class="status">есть несохранённые изменения</span>
      <span v-else-if="saved" class="status" data-testid="settings-json-saved">сохранено</span>
      <VButton label="Сохранить" size="small" :disabled="!dirty" title="Ctrl+S" data-testid="settings-json-save" @click="save" />
    </div>
    <VMessage v-if="error" severity="error" size="small" data-testid="settings-json-error">{{ error }}</VMessage>
    <VTextarea :model-value="draft" class="area" spellcheck="false" wrap="off" aria-label="settings.json" data-testid="settings-json-editor" @update:model-value="onInput" />
  </div>
</template>

<style scoped>
.editor {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--vr-space-3);
  min-height: 0;
  padding: var(--vr-space-4) var(--vr-space-6);
}

.bar {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
}

.path {
  flex: 1;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.status {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.area {
  flex: 1;
  min-height: 0;
  resize: none;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  line-height: 1.5;
  tab-size: 2;
  white-space: pre;
}
</style>
