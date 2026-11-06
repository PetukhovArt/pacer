<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { VButton, VMessage, VTextarea } from '@ui/kit';
import { useI18n } from '../i18n';

const { t } = useI18n();

const props = withDefaults(defineProps<{ name: string; text: string; save: (candidate: string) => string | null; empty?: string; testPrefix?: string }>(), {
  empty: '{}\n',
  testPrefix: 'settings',
});

const EMPTY = props.empty;
const text = computed(() => props.text);
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

function commit() {
  const problem = props.save(draft.value);
  error.value = problem;
  saved.value = problem === null;
}

function onKey(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.code === 'KeyS') {
    event.preventDefault();
    commit();
  }
}
</script>

<template>
  <div class="editor" @keydown="onKey">
    <div class="bar">
      <span class="path">~/.veyra/{{ name }}</span>
      <span v-if="dirty" class="status">{{ t('settings.json.unsaved') }}</span>
      <span v-else-if="saved" class="status" :data-testid="`${testPrefix}-json-saved`">{{ t('settings.json.saved') }}</span>
      <VButton :label="t('common.save')" size="small" :disabled="!dirty" title="Ctrl+S" :data-testid="`${testPrefix}-json-save`" @click="commit" />
    </div>
    <VMessage v-if="error" severity="error" size="small" :data-testid="`${testPrefix}-json-error`">{{ error }}</VMessage>
    <VTextarea :model-value="draft" class="area" spellcheck="false" wrap="off" :aria-label="name" :data-testid="`${testPrefix}-json-editor`" @update:model-value="onInput" />
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
