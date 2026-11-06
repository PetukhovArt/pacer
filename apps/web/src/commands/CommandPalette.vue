<script setup lang="ts">
import { computed, nextTick, onMounted, shallowRef, useTemplateRef } from 'vue';
import { useI18n } from '../i18n';
import { paletteOpen, readContext } from './context';
import { fuzzyScore } from './fuzzy';
import { shortcutLabel } from './keymap';
import { usePickerList } from './usePickerList';
import { isAvailable, listActions, recentActionIds, runAction, type Action } from './registry';

const { t } = useI18n();
const input = useTemplateRef<HTMLInputElement>('input');
const list = useTemplateRef<HTMLElement>('list');
const query = shallowRef('');
const previousFocus = document.activeElement;
const context = readContext(previousFocus);

const available = computed(() => listActions().filter((action) => isAvailable(action, context)));

const results = computed<readonly Action[]>(() => {
  const recent = recentActionIds.value;
  const rank = (action: Action) => {
    const position = recent.indexOf(action.id);
    return position === -1 ? Number.MAX_SAFE_INTEGER : position;
  };
  const text = query.value.trim();
  if (!text) return [...available.value].sort((a, b) => rank(a) - rank(b));
  return available.value
    .map((action, order) => ({ action, order, score: fuzzyScore(text, `${t(action.category)} ${t(action.title)} ${action.id}`) }))
    .filter((item): item is { action: Action; order: number; score: number } => item.score !== null)
    .sort((a, b) => b.score - a.score || rank(a.action) - rank(b.action) || a.order - b.order)
    .map((item) => item.action);
});

function close(restoreFocus: boolean) {
  paletteOpen.value = false;
  if (restoreFocus && previousFocus instanceof HTMLElement && previousFocus.isConnected) void nextTick(() => previousFocus.focus());
}

function choose(action: Action | undefined) {
  if (!action) return;
  close(false);
  void nextTick(() => runAction(action.id));
}

const { index, onKey } = usePickerList({ results, query, list, choose, close: () => close(true) });

onMounted(() => input.value?.focus());
</script>

<template>
  <div class="overlay" data-testid="command-palette" @pointerdown.self="close(true)">
    <div class="panel" role="dialog" :aria-label="t('commands.palette.label')">
      <input
        ref="input"
        v-model="query"
        class="query"
        type="text"
        :placeholder="t('commands.palette.placeholder')"
        :aria-label="t('commands.palette.search')"
        data-testid="command-palette-input"
        @keydown="onKey"
      />
      <ul ref="list" v-auto-animate class="results" role="listbox">
        <li
          v-for="(action, position) in results"
          :key="action.id"
          class="item"
          role="option"
          :aria-selected="position === index"
          :data-current="position === index"
          :data-action-id="action.id"
          data-testid="command-palette-item"
          @pointermove="index = position"
          @click="choose(action)"
        >
          <span class="category">{{ t(action.category) }}</span>
          <span class="title">{{ t(action.title) }}</span>
          <kbd v-if="shortcutLabel(action.id)" class="shortcut">{{ shortcutLabel(action.id) }}</kbd>
        </li>
        <li v-if="!results.length" class="none">{{ t('commands.palette.empty') }}</li>
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

.palette-enter-active {
  animation: vr-fade-in var(--vr-motion-overlay-in) var(--vr-motion-ease-standard) both;
}

.palette-enter-active .panel {
  animation: vr-overlay-in var(--vr-motion-overlay-in) var(--vr-motion-ease-standard) both;
}

.palette-leave-active {
  pointer-events: none;
  animation: vr-fade-out var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate) both;
}

.palette-leave-active .panel {
  animation: vr-overlay-out var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate) both;
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
  gap: var(--vr-space-3);
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-3);
  white-space: nowrap;
  cursor: default;
  border-radius: var(--vr-radius-md);
}

.item[data-current='true'] {
  background: var(--vr-color-nav-active);
}

.category {
  flex: none;
  min-width: 5rem;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--vr-color-text);
  text-overflow: ellipsis;
}

.shortcut {
  flex: none;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.none {
  padding: var(--vr-space-3);
  color: var(--vr-color-text-muted);
}
</style>
