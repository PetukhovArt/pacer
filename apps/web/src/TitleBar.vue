<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { Copy, Minus, Square, X } from 'lucide-vue-next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useI18n } from './i18n';
import { view } from './nav';

const { t } = useI18n();
const win = getCurrentWindow();
const maximized = ref(false);
let stopResize: (() => void) | undefined;

const title = computed(() => {
  switch (view.value.kind) {
    case 'agents':
      return t('shell.nav.agents');
    case 'reviews':
      return t('shell.nav.reviews');
    case 'jira':
      return t('shell.nav.jira');
    case 'confluence':
      return t('shell.nav.confluence');
    case 'settings':
      return t('shell.settings');
    case 'topic':
      return t('shell.topics');
    default:
      return t('shell.tasks');
  }
});

async function sync() {
  maximized.value = await win.isMaximized();
}

onMounted(async () => {
  await sync();
  stopResize = await win.onResized(() => void sync());
});

onUnmounted(() => stopResize?.());
</script>

<template>
  <header class="titlebar" data-tauri-drag-region data-testid="titlebar">
    <svg class="mark" data-tauri-drag-region viewBox="0 0 96 96" width="16" height="16" aria-hidden="true">
      <circle class="ring" cx="48" cy="48" r="46" />
      <path class="stroke" d="M17 25 L48 78 L79 25" />
      <path class="stroke" d="M33 25 L48 51 L63 25" />
      <circle class="node" cx="48" cy="13" r="4" />
    </svg>
    <span class="title" data-tauri-drag-region data-testid="titlebar-title">{{ title }}</span>
    <div class="controls">
      <button type="button" class="ctl" data-testid="titlebar-minimize" :aria-label="t('shell.window.minimize')" :title="t('shell.window.minimize')" @click="win.minimize()">
        <Minus :size="16" :stroke-width="1.5" />
      </button>
      <button
        type="button"
        class="ctl"
        data-testid="titlebar-maximize"
        :data-maximized="maximized"
        :aria-label="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
        :title="maximized ? t('shell.window.restore') : t('shell.window.maximize')"
        @click="win.toggleMaximize()"
      >
        <Copy v-if="maximized" :size="13" :stroke-width="1.5" />
        <Square v-else :size="12" :stroke-width="1.5" />
      </button>
      <button type="button" class="ctl close" data-testid="titlebar-close" :aria-label="t('shell.window.close')" :title="t('shell.window.close')" @click="win.close()">
        <X :size="16" :stroke-width="1.5" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  height: 32px;
  padding-left: var(--vr-space-3);
  user-select: none;
  -webkit-user-select: none;
}

.mark {
  flex: none;
  fill: none;
}

.ring {
  stroke: var(--vr-color-nav-muted);
  stroke-width: 4;
}

.stroke {
  stroke: var(--vr-color-text);
  stroke-width: 9;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.node {
  fill: var(--vr-color-accent);
}

.title {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.controls {
  display: flex;
  height: 100%;
  margin-left: auto;
}

.ctl {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 100%;
  padding: 0;
  color: var(--vr-color-text);
  background: transparent;
  border: 0;
  border-radius: 0;
}

.ctl:hover {
  background: var(--vr-color-nav-hover);
}

.ctl:active {
  background: var(--vr-color-nav-active);
}

.ctl.close:hover {
  color: var(--vr-color-accent-contrast);
  background: var(--vr-color-danger);
}
</style>
