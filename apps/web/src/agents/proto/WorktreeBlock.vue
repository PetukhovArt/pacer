<script setup lang="ts">
import { shallowRef } from 'vue';
import { ChevronRight, GitBranch, Plus } from 'lucide-vue-next';
import WorktreeBadge from '../../forge/WorktreeBadge.vue';
import { openReview } from '../../nav';
import type { SessionNode, WorktreeNode } from '../pacer';
import ArchivedRow from '../ArchivedRow.vue';
import SessionRow from '../SessionRow.vue';

defineProps<{ worktree: WorktreeNode; activeKey: string | null }>();
const emit = defineEmits<{ open: [session: SessionNode]; create: [] }>();

const archiveOpen = shallowRef(false);
</script>

<template>
  <div class="worktree" data-testid="worktree-row" :data-worktree-id="worktree.id">
    <div class="branch">
      <GitBranch :size="12" class="branch-icon" />
      <span class="branch-name" :title="worktree.branch">{{ worktree.branch }}</span>
      <WorktreeBadge :path="worktree.path" :branch="worktree.branch" @open="openReview" />
      <button class="add" title="Новая сессия" aria-label="Новая сессия" data-testid="worktree-new-session" @click="emit('create')"><Plus :size="14" /></button>
    </div>
    <div class="sessions">
      <SessionRow v-for="session in worktree.sessions" :key="session.key" :session="session" :active="session.key === activeKey" @open="emit('open', session)" />
      <template v-if="worktree.archived.length">
        <button class="archive-toggle" :aria-expanded="archiveOpen" data-testid="archive-toggle" @click="archiveOpen = !archiveOpen">
          <ChevronRight :size="12" class="chevron" />
          <span>Архив ({{ worktree.archived.length }})</span>
        </button>
        <template v-if="archiveOpen">
          <ArchivedRow v-for="session in worktree.archived" :key="session.key" :session="session" />
        </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.worktree + .worktree {
  margin-top: var(--vr-space-2);
}

.branch {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  padding: 0 var(--vr-space-1) 0 var(--vr-space-2);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  line-height: var(--vr-size-row);
  color: var(--vr-color-text-muted);
}

.branch-icon {
  flex: none;
}

.branch-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.add {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.add:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.sessions {
  margin-left: var(--vr-space-3);
  padding-left: var(--vr-space-1);
  border-left: 1px solid var(--vr-color-border);
}

.sessions :deep(.row) {
  padding-left: var(--vr-space-2);
}

.archive-toggle {
  display: flex;
  align-items: center;
  gap: var(--vr-space-1);
  width: 100%;
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  text-align: left;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.archive-toggle:hover {
  background: var(--vr-color-nav-hover);
}

.chevron {
  transition: transform 0.1s;
}

.archive-toggle[aria-expanded='true'] .chevron {
  transform: rotate(90deg);
}
</style>
