<script setup lang="ts">
import { computed, nextTick, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { ChevronRight } from 'lucide-vue-next';
import type { SessionNode } from '../pacer';
import StatusDot from '../StatusDot.vue';
import type { SessionPath } from './nav';

const props = defineProps<{ entries: readonly SessionPath[] }>();
const emit = defineEmits<{ select: [session: SessionNode]; close: [] }>();

const SEPARATORS = ' ›-_/.';

const input = useTemplateRef<HTMLInputElement>('input');
const list = useTemplateRef<HTMLElement>('list');
const query = shallowRef('');
const index = shallowRef(0);

function subsequenceScore(word: string, text: string): number | null {
  let matched = 0;
  let last = -2;
  let score = 0;
  for (let position = 0; position < text.length && matched < word.length; position++) {
    if (text[position] !== word[matched]) continue;
    score += position === last + 1 ? 3 : 1;
    if (position === 0 || SEPARATORS.includes(text[position - 1] ?? '')) score += 2;
    last = position;
    matched++;
  }
  return matched === word.length ? score : null;
}

function fullText(entry: SessionPath): string {
  return `${entry.workspace.name} › ${entry.project.name} › ${entry.worktree.branch} › ${entry.session.name}`.toLowerCase();
}

const results = computed(() => {
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [...props.entries];
  return props.entries
    .map((entry, order) => {
      const text = fullText(entry);
      let total = 0;
      for (const word of words) {
        const score = subsequenceScore(word, text);
        if (score === null) return null;
        total += score;
      }
      return { entry, order, total };
    })
    .filter((item): item is { entry: SessionPath; order: number; total: number } => item !== null)
    .sort((a, b) => b.total - a.total || a.order - b.order)
    .map((item) => item.entry);
});

watch(query, () => {
  index.value = 0;
});

function move(delta: number) {
  const count = results.value.length;
  if (!count) return;
  index.value = (index.value + delta + count) % count;
  void nextTick(() => list.value?.querySelector('[data-current="true"]')?.scrollIntoView({ block: 'nearest' }));
}

function choose(entry: SessionPath | undefined) {
  if (entry) emit('select', entry.session);
}

function onKey(event: KeyboardEvent) {
  switch (event.key) {
    case 'ArrowDown':
      move(1);
      break;
    case 'ArrowUp':
      move(-1);
      break;
    case 'Enter':
      choose(results.value[index.value]);
      break;
    case 'Escape':
      emit('close');
      break;
    default:
      return;
  }
  event.preventDefault();
}

onMounted(() => input.value?.focus());
</script>

<template>
  <div class="overlay" data-testid="quick-switcher" @pointerdown.self="emit('close')">
    <div class="panel" role="dialog" aria-label="Быстрый переход к сессии">
      <input
        ref="input"
        v-model="query"
        class="query"
        type="text"
        placeholder="workspace › проект › ветка › сессия"
        aria-label="Поиск сессии"
        data-testid="quick-switcher-input"
        @keydown="onKey"
      />
      <ul ref="list" class="results" role="listbox">
        <li
          v-for="(entry, position) in results"
          :key="entry.session.key"
          class="item"
          role="option"
          :aria-selected="position === index"
          :data-current="position === index"
          data-testid="quick-switcher-item"
          :data-session-id="entry.session.id"
          @pointermove="index = position"
          @click="choose(entry)"
        >
          <StatusDot :status="entry.session.status" :unseen="entry.session.unseen" />
          <span class="path">{{ entry.workspace.name }}</span>
          <ChevronRight :size="11" />
          <span class="path">{{ entry.project.name }}</span>
          <ChevronRight :size="11" />
          <span class="path mono">{{ entry.worktree.branch }}</span>
          <ChevronRight :size="11" />
          <span class="session">{{ entry.session.name }}</span>
        </li>
        <li v-if="!results.length" class="none">Ничего не найдено</li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
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

.none {
  padding: var(--vr-space-3);
  color: var(--vr-color-text-muted);
}
</style>
