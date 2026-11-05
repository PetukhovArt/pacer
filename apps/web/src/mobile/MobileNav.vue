<script setup lang="ts">
import { LayoutPanelLeft, ListTodo, Settings } from 'lucide-vue-next';
import { useI18n } from '../i18n';
import { navigate, view, type View } from '../nav';
import { useDock } from '../workbench/dock';

const { t } = useI18n();
const { detail } = useDock();

const ITEMS = [
  { kind: 'agents', label: 'shell.nav.agents', icon: LayoutPanelLeft },
  { kind: 'tasks', label: 'shell.nav.tasks', icon: ListTodo },
  { kind: 'settings', label: 'shell.settings', icon: Settings },
] as const satisfies readonly { kind: View['kind']; label: string; icon: unknown }[];

const isActive = (kind: View['kind']) => view.value.kind === kind || (kind === 'tasks' && (view.value.kind === 'task' || view.value.kind === 'topic'));

function go(kind: (typeof ITEMS)[number]['kind']) {
  if (kind === 'agents' && view.value.kind === 'agents') detail.value = false;
  else void navigate({ kind });
}
</script>

<template>
  <nav class="nav" :aria-label="t('mobile.navLabel')" data-testid="mobile-nav">
    <button v-for="item in ITEMS" :key="item.kind" class="item" :data-active="isActive(item.kind)" :data-testid="`mobile-nav-${item.kind}`" @click="go(item.kind)">
      <component :is="item.icon" :size="20" />
      <span>{{ t(item.label) }}</span>
    </button>
  </nav>
</template>

<style scoped>
.nav {
  display: flex;
  flex: none;
  padding: var(--vr-space-1) var(--vr-space-1) calc(var(--vr-space-1) + env(safe-area-inset-bottom));
  border-top: 1px solid var(--vr-color-border);
}

.item {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: var(--vr-space-1) 0;
  font: inherit;
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.item[data-active='true'] {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-active);
}
</style>
