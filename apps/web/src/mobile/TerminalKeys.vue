<script setup lang="ts">
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, CornerDownLeft } from 'lucide-vue-next';
import { sendInput, type SessionRef } from '../agents/pacer';
import { useI18n } from '../i18n';
import { ctrlArmed } from './keys';

const props = defineProps<{ session: SessionRef }>();

const { t } = useI18n();

const ESC = '\x1b';

const KEYS = [
  { id: 'esc', data: ESC, label: 'mobile.key.esc', icon: null },
  { id: 'tab', data: '\t', label: 'mobile.key.tab', icon: null },
  { id: 'up', data: `${ESC}[A`, label: 'mobile.key.up', icon: ArrowUp },
  { id: 'down', data: `${ESC}[B`, label: 'mobile.key.down', icon: ArrowDown },
  { id: 'left', data: `${ESC}[D`, label: 'mobile.key.left', icon: ArrowLeft },
  { id: 'right', data: `${ESC}[C`, label: 'mobile.key.right', icon: ArrowRight },
  { id: 'newline', data: `${ESC}\r`, label: 'mobile.key.newline', icon: CornerDownLeft },
] as const;

function press(data: string) {
  ctrlArmed.value = false;
  void sendInput(props.session, data);
}
</script>

<template>
  <div class="keys" role="toolbar" :aria-label="t('mobile.keysLabel')" data-testid="mobile-terminal-keys" @pointerdown.prevent>
    <button class="key" :data-armed="ctrlArmed" :aria-pressed="ctrlArmed" data-testid="mobile-key-ctrl" @click="ctrlArmed = !ctrlArmed">{{ t('mobile.key.ctrl') }}</button>
    <button v-for="key in KEYS" :key="key.id" class="key" :aria-label="t(key.label)" :data-testid="`mobile-key-${key.id}`" @click="press(key.data)">
      <component :is="key.icon" v-if="key.icon" :size="16" />
      <template v-else>{{ t(key.label) }}</template>
    </button>
  </div>
</template>

<style scoped>
.keys {
  display: flex;
  flex: none;
  gap: var(--vr-space-1);
  padding: var(--vr-space-1);
  overflow-x: auto;
  background: var(--vr-color-surface);
  border-top: 1px solid var(--vr-color-border);
}

.key {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  min-width: 2.75rem;
  height: 2.25rem;
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: var(--vr-color-chip-bg);
  border: 0;
  border-radius: var(--vr-radius-md);
}

.key[data-armed='true'] {
  color: var(--vr-color-surface);
  background: var(--vr-color-text);
}
</style>
