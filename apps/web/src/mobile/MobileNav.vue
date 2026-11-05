<script setup lang="ts">
import { GitPullRequest, ListTodo, Settings, SquareTerminal } from 'lucide-vue-next';
import { useI18n } from '../i18n';
import { navigate, view, type View } from '../nav';

const { t } = useI18n();

const ITEMS = [
  { kind: 'tasks', label: 'shell.nav.tasks', icon: ListTodo },
  { kind: 'agents', label: 'shell.nav.agents', icon: SquareTerminal },
  { kind: 'reviews', label: 'shell.nav.reviews', icon: GitPullRequest },
  { kind: 'settings', label: 'shell.settings', icon: Settings },
] as const satisfies readonly { kind: View['kind']; label: string; icon: unknown }[];

const isActive = (kind: View['kind']) => view.value.kind === kind || (kind === 'tasks' && (view.value.kind === 'task' || view.value.kind === 'topic'));
</script>

<template>
  <nav class="nav" :aria-label="t('mobile.navLabel')" data-testid="mobile-nav">
    <button v-for="item in ITEMS" :key="item.kind" class="item" :data-active="isActive(item.kind)" :data-testid="`mobile-nav-${item.kind}`" @click="navigate({ kind: item.kind })">
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
