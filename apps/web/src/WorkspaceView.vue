<script setup lang="ts">
import { watch } from 'vue';
import AgentsView from './agents/AgentsView.vue';
import CommandPalette from './commands/CommandPalette.vue';
import { paletteOpen } from './commands/context';
import UnsavedDialog from './files/UnsavedDialog.vue';
import HotHost from './hot/HotHost.vue';
import MobileNav from './mobile/MobileNav.vue';
import { useIsMobile } from './mobile/useIsMobile';
import Sidebar from './Sidebar.vue';
import TitleBar from './TitleBar.vue';
import { installTray } from './tray';
import UpdateDialog from './update/UpdateDialog.vue';
import { installUpdate } from './update/state';
import { view, type View } from './nav';
import { isDark, toggleTheme } from './settings/appearance';
import SettingsView from './settings/SettingsView.vue';
import ConfluenceView from './views/ConfluenceView.vue';
import JiraView from './views/JiraView.vue';
import { useDock } from './workbench/dock';
import QuickOpen from './workbench/QuickOpen.vue';

const inTauri = '__TAURI_INTERNALS__' in window;
const mobile = useIsMobile();
const viewKey = 'berth.view.v2';
const dock = useDock();
installTray();
installUpdate();

try {
  view.value = JSON.parse(localStorage.getItem(viewKey) ?? '') as View;
} catch {
  view.value = { kind: 'agents' };
}
watch(
  view,
  (value) => {
    if (value.kind === 'reviews') {
      view.value = { kind: 'agents' };
      if (value.open) dock.openReview(value.open);
      return;
    }
    localStorage.setItem(viewKey, JSON.stringify(value));
  },
  { immediate: true },
);
</script>

<template>
  <div class="shell">
    <TitleBar v-if="inTauri" />
    <div class="body" :data-mobile="mobile">
      <Sidebar v-if="!mobile" :dark="isDark" @theme="toggleTheme" />
      <main class="card vt-main">
        <KeepAlive include="AgentsView">
          <AgentsView v-if="view.kind === 'agents'" />
          <JiraView v-else-if="view.kind === 'jira'" />
          <ConfluenceView v-else-if="view.kind === 'confluence'" />
          <SettingsView v-else />
        </KeepAlive>
      </main>
    </div>
    <MobileNav v-if="mobile" />
    <QuickOpen />
    <UnsavedDialog />
    <UpdateDialog v-if="inTauri" />
    <HotHost />
    <Transition name="palette">
      <CommandPalette v-if="paletteOpen" />
    </Transition>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  overflow: hidden;
}

.body {
  display: grid;
  flex: 1;
  grid-template-columns: auto minmax(0, 1fr);
  min-height: 0;
  min-width: 0;
}

.body[data-mobile='true'] {
  grid-template-columns: minmax(0, 1fr);
}

.body[data-mobile='true'] .card {
  margin: 0;
  border-radius: 0;
  box-shadow: none;
}

.card {
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
  margin: var(--vr-space-2) var(--vr-space-2) var(--vr-space-2) 1px;
  overflow: hidden;
  background: var(--vr-color-surface);
  border-radius: var(--vr-radius-2xl);
  box-shadow: var(--vr-elevation-card);
}
</style>
