<script setup lang="ts">
import { computed, onMounted, shallowRef, useTemplateRef } from 'vue';
import { ChevronRight, FileText } from 'lucide-vue-next';
import type { SessionNode } from '../pacer';
import { subsequenceScore } from '../../commands/fuzzy';
import { usePickerList } from '../../commands/usePickerList';
import { t } from '../../i18n';
import StatusDot from '../StatusDot.vue';
import type { MarkdownEntry } from './markdownFiles';
import type { SessionPath } from './nav';

type Item = { kind: 'session'; key: string; entry: SessionPath } | { kind: 'file'; key: string; file: MarkdownEntry };

const props = withDefaults(defineProps<{ entries: readonly SessionPath[]; files?: readonly MarkdownEntry[]; fileScope?: string }>(), { files: () => [], fileScope: '' });
const emit = defineEmits<{ select: [session: SessionNode]; 'select-file': [path: string]; close: [] }>();

const input = useTemplateRef<HTMLInputElement>('input');
const list = useTemplateRef<HTMLElement>('list');
const query = shallowRef('');

function fullText(entry: SessionPath): string {
  return `${entry.workspace.name} › ${entry.project.name} › ${entry.worktree.branch} › ${entry.session.name}`.toLowerCase();
}

const results = computed<Item[]>(() => {
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean);
  const sessions: Item[] = props.entries.map((entry) => ({ kind: 'session', key: entry.session.key, entry }));
  if (!words.length) return sessions;
  const files: Item[] = props.files.map((file) => ({ kind: 'file', key: file.path, file }));
  return [...sessions, ...files]
    .map((item, order) => {
      const text = item.kind === 'session' ? fullText(item.entry) : item.file.rel.toLowerCase();
      let total = 0;
      for (const word of words) {
        const score = subsequenceScore(word, text);
        if (score === null) return null;
        total += score;
      }
      return { item, order, total };
    })
    .filter((match): match is { item: Item; order: number; total: number } => match !== null)
    .sort((a, b) => b.total - a.total || a.order - b.order)
    .map((match) => match.item);
});

function choose(item: Item | undefined) {
  if (!item) return;
  if (item.kind === 'session') emit('select', item.entry.session);
  else emit('select-file', item.file.path);
}

const { index, onKey } = usePickerList({ results, query, list, choose, close: () => emit('close') });

onMounted(() => input.value?.focus());
</script>

<template>
  <div class="overlay" data-testid="quick-switcher" @pointerdown.self="emit('close')">
    <div class="panel" role="dialog" :aria-label="t('files.switcher.dialog')">
      <input
        ref="input"
        v-model="query"
        class="query"
        type="text"
        :placeholder="fileScope ? t('files.switcher.placeholderFiles', { scope: fileScope }) : t('agents.layouts.pathHint')"
        :aria-label="t('files.switcher.search')"
        data-testid="quick-switcher-input"
        @keydown="onKey"
      />
      <ul ref="list" v-auto-animate class="results" role="listbox">
        <li
          v-for="(item, position) in results"
          :key="item.key"
          class="item"
          role="option"
          :aria-selected="position === index"
          :data-current="position === index"
          :data-testid="item.kind === 'session' ? 'quick-switcher-item' : 'quick-switcher-file'"
          :data-session-id="item.kind === 'session' ? item.entry.session.id : undefined"
          @pointermove="index = position"
          @click="choose(item)"
        >
          <template v-if="item.kind === 'session'">
            <StatusDot :status="item.entry.session.status" :unseen="item.entry.session.unseen" />
            <span class="path">{{ item.entry.workspace.name }}</span>
            <ChevronRight :size="11" />
            <span class="path">{{ item.entry.project.name }}</span>
            <ChevronRight :size="11" />
            <span class="path mono">{{ item.entry.worktree.branch }}</span>
            <ChevronRight :size="11" />
            <span class="session">{{ item.entry.session.name }}</span>
          </template>
          <template v-else>
            <FileText :size="14" class="file-icon" />
            <span class="session">{{ item.file.name }}</span>
            <span class="path mono">{{ item.file.rel }}</span>
          </template>
        </li>
        <li v-if="!results.length" class="none">{{ t('common.noResults') }}</li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.qs-enter-active {
  transition: opacity var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.qs-leave-active {
  transition: opacity var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate);
}

.qs-enter-active .panel {
  transition: transform var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.qs-enter-from,
.qs-leave-to {
  opacity: 0;
}

.qs-enter-from .panel {
  transform: scale(0.98);
}

.overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  justify-content: center;
  padding-top: 14vh;
  background: var(--vr-palette-black-a-40);
}

.panel {
  align-self: flex-start;
  width: min(680px, 90%);
  overflow: hidden;
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-shadow-popover);
}

.query {
  box-sizing: border-box;
  width: 100%;
  padding: var(--vr-space-3) var(--vr-space-4);
  font: inherit;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-bottom: 1px solid var(--vr-color-border);
  outline: none;
}

.results {
  max-height: 50vh;
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
  padding: 0 var(--vr-space-3);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  border-radius: var(--vr-radius-md);
}

.item[data-current='true'] {
  background: var(--vr-color-nav-active);
}

.path {
  overflow: hidden;
  text-overflow: ellipsis;
}

.mono {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.session {
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  color: var(--vr-color-text);
  text-overflow: ellipsis;
}

.file-icon {
  flex: none;
}

.none {
  padding: var(--vr-space-3);
  color: var(--vr-color-text-muted);
}
</style>
