<script setup lang="ts">
import { computed, onMounted, shallowRef, useTemplateRef } from 'vue';
import { ChevronRight, FileText, GitPullRequest, Ticket, Zap } from 'lucide-vue-next';
import { subsequenceScore } from '../../commands/fuzzy';
import { usePickerList } from '../../commands/usePickerList';
import PipelineIcon from '../../forge/PipelineIcon.vue';
import { t } from '../../i18n';
import type { QuickItem, WorkNode } from '../../workbench/tree';
import { externalDot } from '../external';
import HarnessIcon from '../HarnessIcon.vue';
import StatusDot from '../StatusDot.vue';
import type { MarkdownEntry } from './markdownFiles';

type QuickAction = { key: string; label: string };
type Item = { kind: 'leaf'; key: string; entry: QuickItem } | { kind: 'file'; key: string; file: MarkdownEntry } | { kind: 'action'; key: string; action: QuickAction };

const props = withDefaults(defineProps<{ entries: readonly QuickItem[]; files?: readonly MarkdownEntry[]; fileScope?: string; actions?: readonly QuickAction[] }>(), { files: () => [], fileScope: '', actions: () => [] });
const emit = defineEmits<{ select: [node: WorkNode]; 'select-file': [path: string]; 'select-action': [key: string]; close: [] }>();

const input = useTemplateRef<HTMLInputElement>('input');
const list = useTemplateRef<HTMLElement>('list');
const query = shallowRef('');

const ICONS = { mr: GitPullRequest, issue: Ticket } as const;
const iconOf = (kind: string) => (kind in ICONS ? ICONS[kind as keyof typeof ICONS] : undefined);

const results = computed<Item[]>(() => {
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean);
  const leaves: Item[] = props.entries.map((entry) => ({ kind: 'leaf', key: entry.key, entry }));
  if (!words.length) return leaves;
  const files: Item[] = props.files.map((file) => ({ kind: 'file', key: file.path, file }));
  const actions: Item[] = props.actions.map((action) => ({ kind: 'action', key: `action:${action.key}`, action }));
  return [...leaves, ...files, ...actions]
    .map((item, order) => {
      const text = item.kind === 'leaf' ? item.entry.search : item.kind === 'file' ? item.file.rel.toLowerCase() : item.action.label.toLowerCase();
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
  if (item.kind === 'leaf') emit('select', item.entry.node);
  else if (item.kind === 'file') emit('select-file', item.file.path);
  else emit('select-action', item.action.key);
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
        :placeholder="fileScope ? t('files.switcher.placeholderFiles', { scope: fileScope }) : t('workbench.quickPath')"
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
          :data-testid="item.kind === 'leaf' ? 'quick-switcher-item' : item.kind === 'file' ? 'quick-switcher-file' : 'quick-switcher-action'"
          :data-kind="item.kind === 'leaf' ? item.entry.node.kind : item.kind"
          :data-session-id="item.kind === 'leaf' ? item.entry.node.data?.session?.id : undefined"
          @pointermove="index = position"
          @click="choose(item)"
        >
          <template v-if="item.kind === 'leaf'">
            <StatusDot v-if="item.entry.node.data?.session" :status="item.entry.node.data.session.status" :unseen="item.entry.node.data.session.unseen" />
            <template v-else-if="item.entry.node.data?.external">
              <StatusDot :status="externalDot(item.entry.node.data.external)" :unseen="false" />
              <HarnessIcon :harness="item.entry.node.data.external.harness" />
            </template>
            <component :is="iconOf(item.entry.node.kind)" v-else-if="iconOf(item.entry.node.kind)" :size="14" class="file-icon" />
            <template v-for="crumb in item.entry.crumbs" :key="crumb">
              <span class="path">{{ crumb }}</span>
              <ChevronRight :size="11" />
            </template>
            <span class="session">{{ item.entry.node.label }}</span>
            <PipelineIcon v-if="item.entry.node.data?.item && item.entry.node.data.item.pipeline !== 'none'" :status="item.entry.node.data.item.pipeline" :size="12" />
          </template>
          <template v-else-if="item.kind === 'action'">
            <Zap :size="14" class="file-icon" />
            <span class="session">{{ item.action.label }}</span>
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
