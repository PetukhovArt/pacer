<script setup lang="ts">
import { computed, nextTick, useTemplateRef, watch } from 'vue';
import { ChevronRight, SquareTerminal } from 'lucide-vue-next';
import HarnessIcon from '../agents/HarnessIcon.vue';
import StatusDot from '../agents/StatusDot.vue';
import { t } from '../i18n';
import { hotDotOf, useHot } from './hot';
import { chooseSwitcher, switcher } from './hotkeys';

const hot = useHot();
const list = useTemplateRef<HTMLElement>('list');

const rows = computed(() => {
  const byKey = new Map(hot.items.value.map((item) => [item.key, item]));
  return (switcher.value?.keys ?? []).flatMap((key) => {
    const item = byKey.get(key);
    return item ? [item] : [];
  });
});

watch(
  () => switcher.value?.index,
  async () => {
    await nextTick();
    list.value?.querySelector('[data-current="true"]')?.scrollIntoView({ block: 'nearest' });
  },
);
</script>

<template>
  <div class="overlay" data-testid="hot-switcher" @pointerdown.self="switcher = null">
    <div class="panel" role="dialog" :aria-label="t('hot.switcher.dialog')">
      <ul ref="list" class="results" role="listbox">
        <li
          v-for="(item, position) in rows"
          :key="item.key"
          class="item"
          role="option"
          :aria-selected="position === switcher?.index"
          :data-current="position === switcher?.index"
          :data-status="item.status"
          data-testid="hot-switcher-item"
          :data-key="item.key"
          @pointermove="switcher && (switcher = { keys: switcher.keys, index: position })"
          @click="chooseSwitcher(position)"
        >
          <StatusDot :status="hotDotOf(item)" :unseen="item.unseen" />
          <SquareTerminal v-if="item.harness === 'shell'" :size="12" class="shell" />
          <HarnessIcon v-else :harness="item.harness" />
          <span class="text">
            <span class="head">
              <span class="title">{{ item.title }}</span>
              <span v-if="item.project || item.branch" class="crumb">
                <template v-if="item.project">{{ item.project }}</template>
                <ChevronRight v-if="item.project && item.branch" :size="11" />
                <span v-if="item.branch" class="branch">{{ item.branch }}</span>
              </span>
            </span>
            <span v-if="item.preview" class="preview">{{ item.preview }}</span>
          </span>
        </li>
        <li v-if="!rows.length" class="none">{{ t('hot.switcher.empty') }}</li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.hs-enter-active {
  transition: opacity var(--vr-motion-overlay-in) var(--vr-motion-ease-standard);
}

.hs-leave-active {
  transition: opacity var(--vr-motion-overlay-out) var(--vr-motion-ease-accelerate);
}

.hs-enter-from,
.hs-leave-to {
  opacity: 0;
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
  width: min(640px, 90%);
  overflow: hidden;
  background: var(--vr-color-surface-raised);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-lg);
  box-shadow: var(--vr-shadow-popover);
}

.results {
  max-height: 60vh;
  margin: 0;
  padding: var(--vr-space-1);
  overflow-y: auto;
  list-style: none;
}

.item {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-height: var(--vr-size-row);
  padding: var(--vr-space-1) var(--vr-space-3);
  color: var(--vr-color-text-muted);
  border-radius: var(--vr-radius-md);
}

.item[data-current='true'] {
  background: var(--vr-color-nav-active);
}

.shell {
  flex: none;
}

.text {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.head {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
  white-space: nowrap;
}

.title {
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  color: var(--vr-color-text);
  text-overflow: ellipsis;
}

.crumb {
  display: inline-flex;
  align-items: center;
  gap: var(--vr-space-1);
  min-width: 0;
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  text-overflow: ellipsis;
}

.branch {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
}

.preview {
  overflow: hidden;
  font-size: var(--vr-font-size-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.none {
  padding: var(--vr-space-3);
  color: var(--vr-color-text-muted);
}
</style>
