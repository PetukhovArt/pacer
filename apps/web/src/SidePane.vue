<script setup lang="ts">
import { shallowRef, useTemplateRef } from 'vue';
import { t } from './i18n';

const props = withDefaults(defineProps<{ storageKey: string; defaultWidth: number; label?: string; handleTestid?: string }>(), {
  handleTestid: 'tree-resize-handle',
});

const MIN_WIDTH = 200;
const KEY_STEP = 16;

const pane = useTemplateRef<HTMLElement>('pane');
const stored = Number(localStorage.getItem(props.storageKey));
const width = shallowRef(stored >= MIN_WIDTH ? stored : props.defaultWidth);

function setWidth(next: number) {
  const max = Math.max(MIN_WIDTH, (pane.value?.parentElement?.clientWidth ?? Number.POSITIVE_INFINITY) / 2);
  width.value = Math.round(Math.min(max, Math.max(MIN_WIDTH, next)));
}

function save() {
  localStorage.setItem(props.storageKey, String(width.value));
}

function startResize(event: PointerEvent) {
  const handle = event.currentTarget;
  if (!(handle instanceof HTMLElement)) return;
  event.preventDefault();
  const startX = event.clientX;
  const startWidth = width.value;
  handle.setPointerCapture(event.pointerId);
  const move = (next: PointerEvent) => setWidth(startWidth + next.clientX - startX);
  const end = () => {
    handle.removeEventListener('pointermove', move);
    handle.removeEventListener('pointerup', end);
    handle.removeEventListener('pointercancel', end);
    save();
  };
  handle.addEventListener('pointermove', move);
  handle.addEventListener('pointerup', end);
  handle.addEventListener('pointercancel', end);
}

function resizeByKey(event: KeyboardEvent) {
  const step = event.key === 'ArrowLeft' ? -KEY_STEP : event.key === 'ArrowRight' ? KEY_STEP : 0;
  if (!step) return;
  event.preventDefault();
  setWidth(width.value + step);
  save();
}
</script>

<template>
  <div ref="pane" class="pane" :style="{ width: `${width}px` }">
    <slot />
    <div
      class="resize-handle"
      role="separator"
      aria-orientation="vertical"
      :aria-label="label"
      tabindex="0"
      :aria-valuenow="width"
      :aria-valuemin="MIN_WIDTH"
      :data-testid="handleTestid"
      @pointerdown="startResize"
      @keydown="resizeByKey"
    />
  </div>
</template>

<style scoped>
.pane {
  position: relative;
  display: flex;
  flex: none;
  flex-direction: column;
  min-width: 0;
  max-width: 50%;
  min-height: 0;
  border-right: 1px solid var(--vr-color-border);
}

.resize-handle {
  position: absolute;
  top: 0;
  right: -3px;
  bottom: 0;
  z-index: 1;
  width: 6px;
  cursor: col-resize;
  touch-action: none;
}

.resize-handle:hover,
.resize-handle:focus-visible {
  outline: none;
  background: var(--vr-color-nav-active);
}
</style>
