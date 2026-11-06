<script setup lang="ts">
import { onBeforeUnmount } from 'vue';
import { hotToast } from './hot';
import { installHotkeys, switcher } from './hotkeys';
import HotOverview from './HotOverview.vue';
import HotSwitcher from './HotSwitcher.vue';
import { hotEnabled } from './view';

const uninstall = installHotkeys();
onBeforeUnmount(uninstall);
</script>

<template>
  <HotOverview v-if="hotEnabled" />
  <Transition name="hs">
    <HotSwitcher v-if="switcher" />
  </Transition>
  <Transition name="ht">
    <p v-if="hotToast" class="toast" role="status" data-testid="hot-toast">{{ hotToast }}</p>
  </Transition>
</template>

<style scoped>
.ht-enter-active {
  transition: opacity var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.ht-leave-active {
  transition: opacity var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate);
}

.ht-enter-from,
.ht-leave-to {
  opacity: 0;
}

.toast {
  position: fixed;
  bottom: var(--vr-space-6);
  left: 50%;
  z-index: 1100;
  margin: 0;
  padding: var(--vr-space-2) var(--vr-space-4);
  font-size: var(--vr-font-size-sm);
  color: var(--vr-color-text);
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-shadow-popover);
  transform: translateX(-50%);
  pointer-events: none;
}
</style>
