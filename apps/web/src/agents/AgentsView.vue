<script setup lang="ts">
import { t } from '../i18n';
import BackBar from '../mobile/BackBar.vue';
import TerminalKeys from '../mobile/TerminalKeys.vue';
import { useIsMobile } from '../mobile/useIsMobile';
import { useDock } from '../workbench/dock';
import ProjectTree from '../workbench/ProjectTree.vue';
import NewSessionForm from './NewSessionForm.vue';
import DockArea from './proto/DockArea.vue';

localStorage.removeItem('veyra.agents.layout');
localStorage.removeItem('veyra.agents.explorer.expanded');

const dock = useDock();
const { onReady } = dock.useHost();
const mobile = useIsMobile();
const { activeKey, activeTitle, activeSession, paths, detail, creatingWorktree } = dock;
</script>

<template>
  <div v-if="!dock.pacer" class="unavailable" data-testid="pacer-banner" data-state="unavailable">{{ t('agents.unavailable') }}</div>
  <div v-else class="workbench" data-testid="workbench" :data-mobile="mobile" :data-detail="detail">
    <ProjectTree v-if="mobile && !detail" class="mobile-tree" with-filter />
    <BackBar v-if="mobile && detail" :title="activeSession?.name ?? activeTitle" @back="detail = false" />
    <DockArea class="dock-slot" :active-key="activeKey" :paths="paths" group-labels @ready="onReady" @open-file="(path, hash, from) => dock.openFile(path, { hash, from })" />
    <TerminalKeys v-if="mobile && detail && activeSession" :session="activeSession.ref" />
    <NewSessionForm v-if="creatingWorktree" :key="creatingWorktree.id" :worktree="creatingWorktree" @close="dock.creatingIn.value = null" @created="dock.onCreated" />
  </div>
</template>

<style scoped>
.unavailable {
  margin: auto;
  color: var(--vr-color-text-muted);
}

.workbench {
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.workbench[data-mobile='true'] {
  flex-direction: column;
}

.workbench[data-mobile='true'][data-detail='false'] > .dock-slot {
  display: none;
}
</style>
