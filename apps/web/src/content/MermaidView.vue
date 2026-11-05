<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { Check, Code, Copy, Maximize2, MoveHorizontal, X, ZoomIn, ZoomOut } from 'lucide-vue-next';
import { t } from '../i18n';
import { renderDiagram } from './mermaid';
import { useDarkTheme } from './theme';

const props = defineProps<{ source: string }>();

const MIN_SCALE = 0.2;
const MAX_SCALE = 6;
const PADDING = 16;
const INLINE_MAX_VH = 0.8;
const LINE_HEIGHT_PX = 16;
const CLOSE_FALLBACK_MS = 400;

const dark = useDarkTheme();
const svg = ref('');
const error = ref('');
const loading = ref(true);
const showSource = ref(false);
const copyState = ref<'idle' | 'ok' | 'fail'>('idle');
const scale = ref(1);
const tx = ref(0);
const ty = ref(0);
const height = ref(240);
const viewport = ref<HTMLElement>();
const stage = ref<HTMLElement>();
const slotEl = ref<HTMLElement>();
const expandButton = ref<HTMLButtonElement>();
const closeButton = ref<HTMLButtonElement>();
const full = ref(false);
const closing = ref(false);
const slotHeight = ref(0);

let run = 0;
let copyTimer = 0;
let closeTimer = 0;
let natural: { w: number; h: number } | null = null;
let drag: { id: number; x: number; y: number } | null = null;

const percent = computed(() => Math.round(scale.value * 100));
const transform = computed(() => `translate(${tx.value}px, ${ty.value}px) scale(${scale.value})`);
const copyLabel = computed(() => (copyState.value === 'ok' ? t('content.mermaid.copied') : copyState.value === 'fail' ? t('content.mermaid.copyFailed') : t('content.mermaid.copySvg')));

function describe(cause: unknown): string {
  if (cause instanceof Error) return cause.message;
  return typeof cause === 'string' ? cause : t('content.mermaid.unknownError');
}

function naturalSize(el: SVGSVGElement): { w: number; h: number } {
  const box = el.viewBox.baseVal;
  if (box && box.width > 0 && box.height > 0) return { w: box.width, h: box.height };
  const rect = el.getBoundingClientRect();
  return { w: rect.width || 600, h: rect.height || 300 };
}

function fit() {
  const el = stage.value?.querySelector('svg');
  const host = viewport.value;
  if (!el || !host) return;
  const { w, h } = naturalSize(el);
  natural = { w, h };
  el.setAttribute('width', String(w));
  el.setAttribute('height', String(h));
  el.style.maxWidth = 'none';
  const available = host.clientWidth - PADDING * 2;
  const next = available > 0 ? Math.min(1, available / w) : 1;
  scale.value = next;
  tx.value = Math.max(PADDING, (host.clientWidth - w * next) / 2);
  if (full.value) {
    ty.value = Math.max(PADDING, (host.clientHeight - h * next) / 2);
  } else {
    ty.value = PADDING;
    height.value = Math.round(Math.min(window.innerHeight * INLINE_MAX_VH, Math.max(160, h * next + PADDING * 2)));
  }
}

function clampAxis(position: number, view: number, content: number): number {
  if (content + PADDING * 2 <= view) return position;
  return Math.min(PADDING, Math.max(view - content - PADDING, position));
}

async function draw() {
  const token = ++run;
  loading.value = true;
  try {
    const out = await renderDiagram(props.source, dark.value);
    if (token !== run) return;
    svg.value = out;
    error.value = '';
  } catch (cause) {
    if (token !== run) return;
    svg.value = '';
    error.value = describe(cause);
  }
  loading.value = false;
  await nextTick();
  fit();
}

function zoomAt(factor: number, x: number, y: number) {
  const next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale.value * factor));
  const ratio = next / scale.value;
  tx.value = x - (x - tx.value) * ratio;
  ty.value = y - (y - ty.value) * ratio;
  scale.value = next;
}

function zoomCenter(factor: number) {
  const host = viewport.value;
  if (host) zoomAt(factor, host.clientWidth / 2, host.clientHeight / 2);
}

function onWheel(event: WheelEvent) {
  const host = viewport.value;
  if (!host) return;
  if (event.ctrlKey) {
    event.preventDefault();
    const rect = host.getBoundingClientRect();
    zoomAt(Math.exp(-event.deltaY * 0.0025), event.clientX - rect.left, event.clientY - rect.top);
    return;
  }
  if (!natural) return;
  const unit = event.deltaMode === 1 ? LINE_HEIGHT_PX : event.deltaMode === 2 ? host.clientHeight : 1;
  let dx = event.deltaX * unit;
  let dy = event.deltaY * unit;
  if (event.shiftKey && dx === 0) {
    dx = dy;
    dy = 0;
  }
  const nextX = dx === 0 ? tx.value : clampAxis(tx.value - dx, host.clientWidth, natural.w * scale.value);
  const nextY = dy === 0 ? ty.value : clampAxis(ty.value - dy, host.clientHeight, natural.h * scale.value);
  const moved = nextX !== tx.value || nextY !== ty.value;
  if (moved || full.value) event.preventDefault();
  tx.value = nextX;
  ty.value = nextY;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape') return;
  event.preventDefault();
  event.stopPropagation();
  closeFull();
}

function onResize() {
  if (full.value) fit();
}

async function openFull() {
  if (full.value) return;
  slotHeight.value = slotEl.value?.offsetHeight ?? 0;
  closing.value = false;
  full.value = true;
  window.addEventListener('keydown', onKeydown, true);
  window.addEventListener('resize', onResize);
  await nextTick();
  fit();
  closeButton.value?.focus();
}

function closeFull() {
  if (!full.value || closing.value) return;
  closing.value = true;
  closeTimer = window.setTimeout(finishClose, CLOSE_FALLBACK_MS);
}

async function finishClose() {
  window.clearTimeout(closeTimer);
  if (!full.value) return;
  window.removeEventListener('keydown', onKeydown, true);
  window.removeEventListener('resize', onResize);
  full.value = false;
  closing.value = false;
  await nextTick();
  fit();
  expandButton.value?.focus();
}

function onLayerAnimationEnd(event: AnimationEvent) {
  if (closing.value && event.target === event.currentTarget) finishClose();
}

function onPointerDown(event: PointerEvent) {
  if (event.button !== 0) return;
  drag = { id: event.pointerId, x: event.clientX, y: event.clientY };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function onPointerMove(event: PointerEvent) {
  if (!drag || drag.id !== event.pointerId) return;
  tx.value += event.clientX - drag.x;
  ty.value += event.clientY - drag.y;
  drag = { id: event.pointerId, x: event.clientX, y: event.clientY };
}

function onPointerUp(event: PointerEvent) {
  if (drag?.id === event.pointerId) drag = null;
}

async function copySvg() {
  window.clearTimeout(copyTimer);
  try {
    await navigator.clipboard.writeText(svg.value);
    copyState.value = 'ok';
  } catch {
    copyState.value = 'fail';
  }
  copyTimer = window.setTimeout(() => (copyState.value = 'idle'), 1600);
}

watch(() => [props.source, dark.value], draw, { immediate: true });

watch(showSource, async (shown) => {
  if (shown) return;
  await nextTick();
  fit();
});

onBeforeUnmount(() => {
  run++;
  window.clearTimeout(copyTimer);
  window.clearTimeout(closeTimer);
  window.removeEventListener('keydown', onKeydown, true);
  window.removeEventListener('resize', onResize);
});
</script>

<template>
  <div ref="slotEl" class="slot" :style="full ? { height: `${slotHeight}px` } : undefined">
  <Teleport to="body" :disabled="!full">
  <div class="layer" :class="{ full, closing }" @animationend="onLayerAnimationEnd">
  <div class="mermaid-view" :class="{ full }" data-testid="mermaid-view" :role="full ? 'dialog' : undefined" :aria-modal="full ? 'true' : undefined">
    <div class="bar">
      <template v-if="svg && !showSource">
        <button type="button" class="btn icon" :title="t('content.mermaid.zoomOut')" :aria-label="t('content.mermaid.zoomOut')" data-testid="mermaid-zoom-out" @click="zoomCenter(1 / 1.25)">
          <ZoomOut :size="14" />
        </button>
        <span class="percent" data-testid="mermaid-zoom-level">{{ percent }}%</span>
        <button type="button" class="btn icon" :title="t('content.mermaid.zoomIn')" :aria-label="t('content.mermaid.zoomIn')" data-testid="mermaid-zoom-in" @click="zoomCenter(1.25)">
          <ZoomIn :size="14" />
        </button>
        <button type="button" class="btn icon" :title="t('content.mermaid.fit')" :aria-label="t('content.mermaid.fit')" data-testid="mermaid-fit" @click="fit">
          <MoveHorizontal :size="14" />
        </button>
      </template>
      <span class="hint" v-if="svg && !showSource">{{ t('content.mermaid.hint') }}</span>
      <span class="spacer" />
      <button v-if="svg" type="button" class="btn" data-testid="mermaid-copy-svg" @click="copySvg">
        <component :is="copyState === 'ok' ? Check : Copy" :size="14" />
        {{ copyLabel }}
      </button>
      <button type="button" class="btn" :class="{ on: showSource }" :aria-pressed="showSource" data-testid="mermaid-source-toggle" @click="showSource = !showSource">
        <Code :size="14" />
        {{ t('content.mermaid.source') }}
      </button>
      <button v-if="svg && !full" ref="expandButton" type="button" class="btn icon" :title="t('content.mermaid.fullscreen')" :aria-label="t('content.mermaid.fullscreen')" data-testid="mermaid-fullscreen" @click="openFull">
        <Maximize2 :size="14" />
      </button>
      <button v-if="full" ref="closeButton" type="button" class="btn icon" :title="t('content.mermaid.close')" :aria-label="t('content.mermaid.close')" data-testid="mermaid-close" @click="closeFull">
        <X :size="14" />
      </button>
    </div>
    <div v-if="error" class="error" role="alert" data-testid="mermaid-error">
      <strong>{{ t('content.mermaid.failed') }}</strong>
      <pre class="message">{{ error }}</pre>
    </div>
    <pre v-if="showSource || error" class="source" data-testid="mermaid-source">{{ source }}</pre>
    <div
      v-show="!showSource && !error"
      ref="viewport"
      class="viewport"
      :style="full ? undefined : { height: `${height}px` }"
      data-testid="mermaid-viewport"
      @wheel="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerUp"
    >
      <div ref="stage" class="stage" :style="{ transform }" v-html="svg" />
      <div v-if="loading" class="loading">{{ t('content.mermaid.loading') }}</div>
    </div>
  </div>
  </div>
  </Teleport>
  </div>
</template>

<style scoped>
.slot {
  display: flow-root;
}

.layer {
  display: contents;
}

.layer.full {
  position: fixed;
  inset: 0;
  z-index: 2000;
  display: flex;
  padding: var(--vr-space-4);
  background: var(--vr-palette-black-a-50);
  animation: vr-fade-in var(--vr-motion-overlay-in) var(--vr-motion-ease-standard) both;
}

.layer.full .mermaid-view {
  animation: vr-overlay-in var(--vr-motion-overlay-in) var(--vr-motion-ease-standard) both;
}

.layer.full.closing {
  pointer-events: none;
  animation: vr-fade-out var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate) both;
}

.layer.full.closing .mermaid-view {
  animation: vr-overlay-out var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate) both;
}

.mermaid-view.full {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  margin: 0;
  background: var(--vr-color-surface-raised);
  box-shadow: var(--vr-shadow-popover);
}

.mermaid-view.full .viewport {
  flex: 1;
  min-height: 0;
}

.mermaid-view.full .source {
  flex: 1;
  max-height: none;
}

.mermaid-view {
  margin: var(--vr-space-3) 0;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  background: var(--vr-color-surface);
  overflow: hidden;
}

.bar {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1) var(--vr-space-2);
  border-bottom: 1px solid var(--vr-color-border);
  background: var(--vr-color-surface-muted);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.spacer {
  flex: 1;
}

.hint {
  margin-left: var(--vr-space-2);
}

.percent {
  min-width: 40px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  height: 24px;
  padding: 0 var(--vr-space-2);
  border: 0;
  border-radius: var(--vr-radius-sm);
  background: transparent;
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-xs);
}

.btn.icon {
  width: 24px;
  padding: 0;
  justify-content: center;
}

.btn:hover,
.btn.on {
  background: var(--vr-color-nav-active);
}

.viewport {
  position: relative;
  overflow: hidden;
  cursor: grab;
  touch-action: none;
  user-select: none;
}

.viewport:active {
  cursor: grabbing;
}

.stage {
  position: absolute;
  top: 0;
  left: 0;
  transform-origin: 0 0;
}

.stage :deep(svg) {
  display: block;
  overflow: visible;
}

.loading {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.error {
  padding: var(--vr-space-2) var(--vr-space-3);
  color: var(--vr-color-danger);
  font-size: var(--vr-font-size-xs);
}

.message {
  margin: var(--vr-space-1) 0 0;
  white-space: pre-wrap;
  font-family: var(--vr-font-mono);
}

.source {
  margin: 0;
  padding: var(--vr-space-3);
  overflow: auto;
  max-height: 480px;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  line-height: 1.55;
  color: var(--vr-color-text);
}
</style>
