<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { SquareTerminal, X } from 'lucide-vue-next';
import HarnessIcon from '../agents/HarnessIcon.vue';
import StatusDot from '../agents/StatusDot.vue';
import { t, useI18n } from '../i18n';
import { useHot, type HotItem } from './hot';
import { hotDot, hotOverviewOpen } from './view';

const hot = useHot();
const { formatRelative } = useI18n();

const grid = useTemplateRef<HTMLElement>('grid');
const index = shallowRef(0);
const now = shallowRef(Date.now());
let ticker: ReturnType<typeof setInterval> | undefined;

const tiles = computed<{ item: HotItem; number: number }[]>(() => {
  const numbered = hot.items.value.map((item, position) => ({ item, number: position + 1 }));
  return [...numbered.filter((tile) => tile.item.status === 'waiting'), ...numbered.filter((tile) => tile.item.status !== 'waiting')];
});

function close() {
  hotOverviewOpen.value = false;
}

function choose(position: number) {
  const tile = tiles.value[position];
  if (!tile) return;
  close();
  hot.open(tile.item.key);
}

function columns(): number {
  const template = grid.value ? getComputedStyle(grid.value).gridTemplateColumns : '';
  return Math.max(1, template.split(' ').filter(Boolean).length);
}

function move(delta: number) {
  const last = tiles.value.length - 1;
  if (last < 0) return;
  index.value = Math.min(last, Math.max(0, index.value + delta));
  void nextTick(() => grid.value?.querySelector('[data-current="true"]')?.scrollIntoView({ block: 'nearest' }));
}

function swallow(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
}

function onKeyDown(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (event.code === 'KeyO' && event.ctrlKey && event.shiftKey && !event.altKey && !event.metaKey) {
    swallow(event);
    hotOverviewOpen.value = !hotOverviewOpen.value;
    return;
  }
  if (!hotOverviewOpen.value || event.ctrlKey || event.altKey || event.metaKey) return;
  switch (event.key) {
    case 'Escape':
      swallow(event);
      close();
      break;
    case 'Enter':
      swallow(event);
      choose(index.value);
      break;
    case 'ArrowRight':
      swallow(event);
      move(1);
      break;
    case 'ArrowLeft':
      swallow(event);
      move(-1);
      break;
    case 'ArrowDown':
      swallow(event);
      move(columns());
      break;
    case 'ArrowUp':
      swallow(event);
      move(-columns());
      break;
    case 'Home':
      swallow(event);
      index.value = 0;
      break;
    case 'End':
      swallow(event);
      index.value = Math.max(0, tiles.value.length - 1);
      break;
    case 'Tab':
      swallow(event);
      move(event.shiftKey ? -1 : 1);
      break;
  }
}

watch(hotOverviewOpen, (open) => {
  clearInterval(ticker);
  if (!open) return;
  index.value = 0;
  now.value = Date.now();
  ticker = setInterval(() => (now.value = Date.now()), 30_000);
  void nextTick(() => grid.value?.focus());
});

watch(
  () => tiles.value.length,
  (length) => {
    if (index.value >= length) index.value = Math.max(0, length - 1);
  },
);

onMounted(() => window.addEventListener('keydown', onKeyDown, true));
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown, true);
  clearInterval(ticker);
});

function ago(item: HotItem): string {
  void now.value;
  return item.lastActivity === null ? '' : formatRelative(item.lastActivity);
}

function crumb(item: HotItem): string {
  return [item.project, item.branch].filter(Boolean).join(' › ');
}
</script>

<template>
  <Transition name="overview">
    <div v-if="hotOverviewOpen" class="overlay" data-testid="hot-overview" @pointerdown.self="close">
      <div class="panel" role="dialog" :aria-label="t('hot.overview.title')">
        <header class="bar">
          <h2 class="heading">{{ t('hot.overview.title') }}</h2>
          <span class="hint">{{ t('hot.overview.hint') }}</span>
          <button class="close" :title="t('hot.overview.close')" :aria-label="t('hot.overview.close')" data-testid="hot-overview-close" @click="close"><X :size="14" /></button>
        </header>
        <div v-if="tiles.length" ref="grid" class="grid" role="listbox" tabindex="-1" :aria-label="t('hot.overview.title')">
          <button
            v-for="(tile, position) in tiles"
            :key="tile.item.key"
            class="tile"
            role="option"
            :aria-selected="position === index"
            :data-current="position === index"
            :data-status="tile.item.status"
            :data-key="tile.item.key"
            data-testid="hot-overview-tile"
            :style="{ '--i': Math.min(position, 14) }"
            tabindex="-1"
            @pointermove="index = position"
            @click="choose(position)"
          >
            <span class="top">
              <StatusDot :status="hotDot(tile.item.status)" :unseen="tile.item.unseen" />
              <SquareTerminal v-if="tile.item.harness === 'shell'" :size="13" class="shell" />
              <HarnessIcon v-else :harness="tile.item.harness" :size="13" />
              <span class="title">{{ tile.item.title }}</span>
              <kbd v-if="tile.number <= 9" class="num">{{ tile.number }}</kbd>
            </span>
            <span class="crumb">{{ crumb(tile.item) || ' ' }}</span>
            <span class="preview" :data-empty="!tile.item.preview">{{ tile.item.preview ?? t('hot.overview.noPreview') }}</span>
            <span class="foot">
              <span class="state">{{ t(`hot.status.${tile.item.status}`) }}</span>
              <span class="ago">{{ ago(tile.item) }}</span>
            </span>
          </button>
        </div>
        <p v-else class="empty" data-testid="hot-overview-empty">{{ t('hot.overview.empty') }}</p>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.overview-enter-active {
  transition: opacity var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.overview-leave-active {
  transition: opacity var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate);
}

.overview-enter-from,
.overview-leave-to {
  opacity: 0;
}

.overview-enter-active .panel {
  transition: transform var(--vr-motion-slow) var(--vr-motion-ease-emphasized);
}

.overview-enter-from .panel {
  transform: scale(0.97);
}

.overlay {
  position: fixed;
  inset: 32px 0 0;
  z-index: 900;
  display: flex;
  padding: var(--vr-space-6);
  background: var(--vr-palette-black-a-40);
}

.panel {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-shadow-popover);
}

.bar {
  display: flex;
  flex: none;
  gap: var(--vr-space-3);
  align-items: center;
  padding: var(--vr-space-2) var(--vr-space-2) var(--vr-space-2) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.heading {
  margin: 0;
  font-size: var(--vr-font-size-sm);
  font-weight: 600;
}

.hint {
  flex: 1;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.close:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.grid {
  display: grid;
  flex: 1;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  grid-auto-rows: min-content;
  gap: var(--vr-space-3);
  min-height: 0;
  padding: var(--vr-space-4);
  overflow-y: auto;
  outline: none;
}

.tile {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
  min-width: 0;
  min-height: 140px;
  padding: var(--vr-space-3);
  font: inherit;
  color: inherit;
  text-align: left;
  background: var(--vr-color-surface);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-elevation-card);
  animation: tile-in var(--vr-motion-slow) var(--vr-motion-ease-emphasized) both;
  animation-delay: calc(var(--i) * var(--vr-motion-fast) * 0.5);
  transition:
    border-color var(--vr-motion-fast) var(--vr-motion-ease-standard),
    background var(--vr-motion-fast) var(--vr-motion-ease-standard);
}

.tile[data-current='true'] {
  background: var(--vr-color-nav-active);
  border-color: var(--vr-color-accent);
}

.tile[data-status='waiting'] {
  border-color: var(--vr-color-danger);
}

.tile[data-status='waiting'][data-current='true'] {
  border-color: var(--vr-color-accent);
}

.top {
  display: flex;
  gap: var(--vr-space-2);
  align-items: center;
  min-width: 0;
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.shell {
  flex: none;
  color: var(--vr-color-text-muted);
}

.num {
  flex: none;
  padding: 0 var(--vr-space-1);
  font-family: inherit;
  font-size: var(--vr-font-size-2xs);
  font-variant-numeric: tabular-nums;
  color: var(--vr-color-text-muted);
  background: var(--vr-color-chip-bg);
  border-radius: var(--vr-radius-sm);
}

.crumb {
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.preview {
  display: -webkit-box;
  flex: 1;
  overflow: hidden;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  line-height: 1.45;
  color: var(--vr-color-text);
  word-break: break-word;
  -webkit-line-clamp: 4;
  line-clamp: 4;
  -webkit-box-orient: vertical;
}

.preview[data-empty='true'] {
  font-family: inherit;
  color: var(--vr-color-text-muted);
}

.foot {
  display: flex;
  gap: var(--vr-space-2);
  justify-content: space-between;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.tile[data-status='waiting'] .state {
  font-weight: 600;
  color: var(--vr-color-danger);
}

.empty {
  margin: 0;
  padding: var(--vr-space-6);
  color: var(--vr-color-text-muted);
}

@keyframes tile-in {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.98);
  }
}

@media (prefers-reduced-motion: reduce) {
  .tile {
    animation: none;
  }
}
</style>
