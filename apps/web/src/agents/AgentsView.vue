<script setup lang="ts">
import { shallowRef } from 'vue';
import { usePacer } from './pacer';
import LayoutA from './proto/LayoutA.vue';
import LayoutB from './proto/LayoutB.vue';
import LayoutC from './proto/LayoutC.vue';

const LAYOUT_KEY = 'veyra.agents.layout';
const LAYOUTS = [
  { id: 'A', title: 'Проводник: единое дерево' },
  { id: 'B', title: 'Рейка workspace и панель проектов' },
  { id: 'C', title: 'Dock-first: навигатор как панель, группа на ветку, Ctrl+P' },
] as const;
type LayoutId = (typeof LAYOUTS)[number]['id'];

const pacer = usePacer();
const stored = localStorage.getItem(LAYOUT_KEY);
const layout = shallowRef<LayoutId>(LAYOUTS.find((item) => item.id === stored)?.id ?? 'A');

function choose(id: LayoutId) {
  layout.value = id;
  localStorage.setItem(LAYOUT_KEY, id);
}
</script>

<template>
  <div v-if="!pacer" class="unavailable" data-testid="pacer-banner" data-state="unavailable">Сессии pacer работают только в десктопном приложении Veyra</div>
  <div v-else class="agents">
    <header class="head">
      <span class="caption">Раскладка</span>
      <div class="switch" role="radiogroup" aria-label="Раскладка экрана агентов" data-testid="layout-switch">
        <button
          v-for="item in LAYOUTS"
          :key="item.id"
          class="option"
          role="radio"
          :aria-checked="layout === item.id"
          :title="item.title"
          :data-testid="`layout-switch-${item.id}`"
          @click="choose(item.id)"
        >
          {{ item.id }}
        </button>
      </div>
      <span class="caption">{{ LAYOUTS.find((item) => item.id === layout)?.title }}</span>
    </header>
    <LayoutA v-if="layout === 'A'" />
    <LayoutB v-else-if="layout === 'B'" />
    <LayoutC v-else />
  </div>
</template>

<style scoped>
.unavailable {
  margin: auto;
  color: var(--vr-color-text-muted);
}

.agents {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-1) var(--vr-space-4);
  border-bottom: 1px solid var(--vr-color-border);
}

.caption {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.switch {
  display: inline-flex;
  padding: 2px;
  background: var(--vr-color-surface-muted);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.option {
  min-width: 28px;
  height: 22px;
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.option:hover {
  color: var(--vr-color-text);
}

.option[aria-checked='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}
</style>
