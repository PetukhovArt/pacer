<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { VPopover } from '@ui/kit';
import { FileText } from 'lucide-vue-next';
import { describeError } from '../../connectors/api';
import { subsequenceScore } from '../../commands/fuzzy';
import { usePickerList } from '../../commands/usePickerList';
import { t } from '../../i18n';
import { listWorktreeMarkdown, type MarkdownEntry } from './markdownFiles';

const props = defineProps<{ root: string }>();
const emit = defineEmits<{ open: [path: string] }>();

const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');
const input = useTemplateRef<HTMLInputElement>('input');
const list = useTemplateRef<HTMLElement>('list');
const query = shallowRef('');
const files = shallowRef<MarkdownEntry[]>([]);
const loaded = shallowRef(false);
const failure = shallowRef<string | null>(null);

const results = computed(() => {
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return files.value;
  return files.value
    .map((file, order) => {
      const text = file.rel.toLowerCase();
      let total = 0;
      for (const word of words) {
        const score = subsequenceScore(word, text);
        if (score === null) return null;
        total += score;
      }
      return { file, order, total };
    })
    .filter((item): item is { file: MarkdownEntry; order: number; total: number } => item !== null)
    .sort((a, b) => b.total - a.total || a.order - b.order)
    .map((item) => item.file);
});

function close() {
  popover.value?.hide();
}

function choose(file: MarkdownEntry | undefined) {
  if (!file) return;
  close();
  emit('open', file.path);
}

const { index, onKey } = usePickerList({ results, query, list, choose, close });

async function load() {
  loaded.value = false;
  failure.value = null;
  try {
    files.value = await listWorktreeMarkdown(props.root);
  } catch (e) {
    files.value = [];
    failure.value = describeError(e);
  }
  loaded.value = true;
}

function toggle(event: Event) {
  popover.value?.toggle(event);
}

function onShow() {
  query.value = '';
  index.value = 0;
  void load();
  void nextTick(() => input.value?.focus());
}
</script>

<template>
  <button class="trigger" type="button" :title="t('files.picker.open')" :aria-label="t('files.picker.open')" aria-haspopup="dialog" data-testid="worktree-markdown" @click="toggle">
    <FileText :size="14" />
  </button>
  <VPopover ref="popover" @show="onShow">
    <div class="picker" role="dialog" :aria-label="t('files.picker.open')" data-testid="markdown-picker">
      <input ref="input" v-model="query" class="query" type="text" :placeholder="t('files.picker.filter')" :aria-label="t('files.picker.filter')" autocomplete="off" data-testid="markdown-picker-input" @keydown="onKey" />
      <p v-if="failure" class="note failure" role="alert">{{ failure }}</p>
      <ul ref="list" class="results" role="listbox">
        <li
          v-for="(file, position) in results"
          :key="file.path"
          class="item"
          role="option"
          :aria-selected="position === index"
          :data-current="position === index"
          :title="file.rel"
          data-testid="markdown-picker-item"
          @pointermove="index = position"
          @click="choose(file)"
        >
          <span class="name">{{ file.name }}</span>
          <span class="rel">{{ file.rel }}</span>
        </li>
        <li v-if="!loaded" class="note">{{ t('common.loading') }}</li>
        <li v-else-if="!results.length && !failure" class="note" data-testid="markdown-picker-empty">{{ files.length ? t('common.noResults') : t('files.picker.none') }}</li>
      </ul>
    </div>
  </VPopover>
</template>

<style scoped>
.trigger {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.trigger:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.picker {
  display: flex;
  flex-direction: column;
  width: 340px;
}

.query {
  box-sizing: border-box;
  width: 100%;
  padding: var(--vr-space-2) var(--vr-space-3);
  font: inherit;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-bottom: 1px solid var(--vr-color-border);
  outline: none;
}

.results {
  max-height: 320px;
  margin: 0;
  padding: var(--vr-space-1);
  overflow-y: auto;
  list-style: none;
}

.item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  white-space: nowrap;
  border-radius: var(--vr-radius-md);
}

.item[data-current='true'] {
  background: var(--vr-color-nav-active);
}

.name {
  flex: none;
  font-weight: 600;
  color: var(--vr-color-text);
}

.rel {
  min-width: 0;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  text-overflow: ellipsis;
}

.note {
  padding: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  list-style: none;
}

.failure {
  margin: 0;
  color: var(--vr-color-danger);
}
</style>
