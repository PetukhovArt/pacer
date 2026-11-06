<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from 'vue';
import { LayoutGrid, Pin, SquareTerminal } from 'lucide-vue-next';
import { VContextMenu } from '@ui/kit';
import { useExternalSessions } from '../agents/external';
import HarnessIcon from '../agents/HarnessIcon.vue';
import StatusDot from '../agents/StatusDot.vue';
import { t } from '../i18n';
import { useDock } from '../workbench/dock';
import { hotDotOf, useHot, type HotItem } from './hot';
import { toggleHotOverview } from './view';

const hot = useHot();
const dock = useDock();
const external = useExternalSessions();

const menu = useTemplateRef<InstanceType<typeof VContextMenu>>('menu');
const row = useTemplateRef<HTMLElement>('row');
const hidden = shallowRef(0);
let observer: ResizeObserver | undefined;
const menuTarget = shallowRef<HotItem | null>(null);
const dragKey = shallowRef<string | null>(null);
const dropAt = shallowRef<number | null>(null);

const activeKey = computed(() => {
  const key = dock.activeKey.value;
  return key ? (external?.terminalOf.value.get(key) ?? key) : null;
});
const menuItems = computed(() => {
  const item = menuTarget.value;
  if (!item) return [];
  return [{ key: item.pinned ? 'unpin' : 'pin', label: item.pinned ? t('hot.bar.unpin') : t('hot.bar.pin'), command: () => hot.pin(item.key, !item.pinned) }];
});

function measure() {
  const chips = row.value ? Array.from(row.value.children) : [];
  const first = chips[0];
  hidden.value = first instanceof HTMLElement ? chips.filter((chip) => chip instanceof HTMLElement && chip.offsetTop > first.offsetTop).length : 0;
}

function showMenu(event: MouseEvent, item: HotItem) {
  menuTarget.value = item;
  menu.value?.show(event);
}

function onDragStart(event: DragEvent, item: HotItem) {
  dragKey.value = item.key;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', item.key);
  }
}

function onDragOver(event: DragEvent, index: number) {
  if (dragKey.value === null) return;
  event.preventDefault();
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  dropAt.value = event.clientX > rect.left + rect.width / 2 ? index + 1 : index;
}

function onDragEnd() {
  dragKey.value = null;
  dropAt.value = null;
}

function onDrop(event: DragEvent) {
  event.preventDefault();
  const key = dragKey.value;
  let target = dropAt.value;
  onDragEnd();
  if (key === null || target === null) return;
  const from = hot.items.value.findIndex((item) => item.key === key);
  if (from < 0) return;
  if (from < target) target -= 1;
  if (target !== from) hot.move(key, target);
}

watch(hot.items, () => void nextTick(measure), { flush: 'post' });
onMounted(() => {
  measure();
  if (row.value) {
    observer = new ResizeObserver(measure);
    observer.observe(row.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <div class="bar" data-tauri-drag-region data-testid="hot-bar">
    <div ref="row" class="row" role="toolbar" :aria-label="t('hot.bar.label')" data-tauri-drag-region data-testid="hot-strip" @dragover.prevent @drop="onDrop">
      <button
        v-for="(item, index) in hot.items.value"
        :key="item.key"
        class="chip"
        draggable="true"
        data-testid="hot-chip"
        :data-key="item.key"
        :data-status="item.status"
        :data-active="item.key === activeKey"
        :data-pinned="item.pinned"
        :data-dragging="dragKey === item.key"
        :data-drop="dropAt === index ? 'before' : dropAt === index + 1 && index === hot.items.value.length - 1 ? 'after' : undefined"
        :title="`${item.title}\n${t(`hot.status.${item.status}`)}`"
        @click="hot.open(item.key)"
        @contextmenu.prevent="showMenu($event, item)"
        @dragstart="onDragStart($event, item)"
        @dragover="onDragOver($event, index)"
        @dragend="onDragEnd"
      >
        <span v-if="index < 9" class="num" aria-hidden="true">{{ index + 1 }}</span>
        <StatusDot :status="hotDotOf(item)" :unseen="item.unseen" />
        <SquareTerminal v-if="item.harness === 'shell'" :size="12" class="shell" />
        <HarnessIcon v-else :harness="item.harness" />
        <span class="title">{{ item.title }}</span>
        <Pin v-if="item.pinned" :size="10" class="pin" />
      </button>
    </div>
    <button v-if="hidden" type="button" class="more" :title="t('hot.bar.more', { count: hidden })" :aria-label="t('hot.bar.more', { count: hidden })" data-testid="hot-more" @click="toggleHotOverview">+{{ hidden }}</button>
    <button type="button" class="grid" :title="t('hot.overview.open')" :aria-label="t('hot.overview.open')" data-testid="hot-overview-open" @click="toggleHotOverview"><LayoutGrid :size="14" :stroke-width="1.5" /></button>
    <VContextMenu ref="menu" :model="menuItems">
      <template #item="{ item, props: itemProps }">
        <a v-bind="itemProps.action" :data-testid="`hot-menu-${item.key}`">{{ item.label }}</a>
      </template>
    </VContextMenu>
  </div>
</template>

<style scoped>
.bar {
  display: flex;
  flex: 1;
  gap: var(--vr-space-1);
  align-items: center;
  min-width: 0;
  height: 100%;
}

.row {
  display: flex;
  flex: 0 1 auto;
  flex-wrap: wrap;
  gap: 0 var(--vr-space-1);
  align-content: flex-start;
  min-width: 0;
  height: 22px;
  overflow: hidden;
}

.more,
.grid {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  height: 22px;
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  background: none;
  border: 1px solid transparent;
  border-radius: var(--vr-radius-md);
}

.more:hover,
.grid:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.grid {
  margin-right: var(--vr-space-2);
}

.chip {
  position: relative;
  display: inline-flex;
  flex: none;
  gap: var(--vr-space-2);
  align-items: center;
  max-width: 160px;
  height: 22px;
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
  white-space: nowrap;
  background: none;
  border: 1px solid transparent;
  border-radius: var(--vr-radius-md);
}

.chip:hover {
  background: var(--vr-color-nav-hover);
}

.chip[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
  border-color: var(--vr-color-border);
}

.chip[data-status='waiting'] {
  animation: hot-chip-pulse 1.8s var(--vr-motion-ease-standard) infinite;
}

.chip[data-dragging='true'] {
  opacity: 0.5;
}

.chip[data-drop='before']::before,
.chip[data-drop='after']::after {
  position: absolute;
  top: 2px;
  bottom: 2px;
  width: 2px;
  content: '';
  background: var(--vr-color-accent);
  border-radius: var(--vr-radius-full);
}

.chip[data-drop='before']::before {
  left: -3px;
}

.chip[data-drop='after']::after {
  right: -3px;
}

.num {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  font-variant-numeric: tabular-nums;
  color: var(--vr-color-text-muted);
}

.title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.shell,
.pin {
  flex: none;
}

@keyframes hot-chip-pulse {
  50% {
    background: var(--vr-color-chip-bg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .chip[data-status='waiting'] {
    animation: none;
  }
}
</style>
