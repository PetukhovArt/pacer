<script setup lang="ts">
import { computed, shallowRef, watch, type Component } from 'vue';
import { ExternalLink, FileJson, FileText, History, Keyboard, Palette, Plug, Search, SlidersHorizontal, Smartphone, SquareTerminal } from 'lucide-vue-next';
import { VButton, VInputText, VMessage } from '@ui/kit';
import ConnectorsPanel from '../connectors/ConnectorsPanel.vue';
import { useI18n } from '../i18n';
import { withViewTransition } from '../motion';
import { requestedSettingsCategory } from '../nav';
import SettingRow from './SettingRow.vue';
import KeymapPanel from './KeymapPanel.vue';
import RemotePanel from './RemotePanel.vue';
import SettingsJsonEditor from './SettingsJsonEditor.vue';
import { CATEGORIES, SETTINGS, SETTING_KEYS } from './schema';
import { useSettings } from './store';
import { describeFailure } from './parse';

const { t } = useI18n();
const { parseError, issues, ioError, text, saveText, openExternal, canOpenExternal } = useSettings();

const query = shallowRef('');
type CategoryId = (typeof CATEGORIES)[number]['id'];
const category = shallowRef<CategoryId>('appearance');
const rawMode = shallowRef(false);

const CATEGORY_ICONS: Record<CategoryId, Component> = { appearance: Palette, terminal: SquareTerminal, sessions: History, files: FileText, connectors: Plug, remote: Smartphone, keymap: Keyboard };

const normalizedQuery = computed(() => query.value.trim().toLowerCase());
watch(normalizedQuery, (value) => {
  if (value) rawMode.value = false;
});
const showKeymap = computed(() => !normalizedQuery.value && category.value === 'keymap');
const showRemote = computed(() => !normalizedQuery.value && category.value === 'remote');
const showConnectors = computed(() => {
  if (!normalizedQuery.value) return category.value === 'connectors';
  const keywords = t('settings.connectorKeywords');
  return normalizedQuery.value.split(/\s+/).every((word) => keywords.includes(word));
});
const groups = computed(() =>
  CATEGORIES.map((entry) => ({
    ...entry,
    keys: SETTING_KEYS.filter((key) => {
      const def = SETTINGS[key];
      if (def.category !== entry.id) return false;
      if (!normalizedQuery.value) return entry.id === category.value;
      return [t(def.label), key, t(def.description)].some((field) => field.toLowerCase().includes(normalizedQuery.value));
    }),
  })).filter((group) => group.keys.length),
);

function applyCategory(id: CategoryId) {
  category.value = id;
  query.value = '';
  rawMode.value = false;
}

function pickCategory(id: CategoryId) {
  void withViewTransition(() => applyCategory(id));
}

function toggleRaw() {
  void withViewTransition(() => {
    rawMode.value = !rawMode.value;
  });
}

watch(
  requestedSettingsCategory,
  (id) => {
    if (!id) return;
    requestedSettingsCategory.value = null;
    const entry = CATEGORIES.find((item) => item.id === id);
    if (entry) applyCategory(entry.id);
  },
  { immediate: true },
);

function onSearchKey(event: KeyboardEvent) {
  if (event.key === 'Escape') query.value = '';
}
</script>

<template>
  <section class="page" data-testid="settings-view" :aria-label="t('settings.title')">
    <header class="header">
      <h1><SlidersHorizontal :size="18" />{{ t('settings.title') }}</h1>
      <label class="search">
        <Search :size="16" />
        <VInputText v-model="query" size="small" :placeholder="t('settings.search')" :aria-label="t('settings.search')" data-testid="settings-search" @keydown="onSearchKey" />
      </label>
      <VButton
        :label="rawMode ? t('settings.backToSettings') : t('settings.openJson')"
        size="small"
        severity="secondary"
        data-testid="settings-open-json"
        @click="toggleRaw"
      >
        <template #icon><FileJson :size="14" /></template>
      </VButton>
      <VButton
        :label="t('settings.openExternal')"
        size="small"
        severity="secondary"
        text
        :disabled="!canOpenExternal"
        :title="canOpenExternal ? t('settings.openExternalTitle') : t('settings.appOnly')"
        data-testid="settings-open-external"
        @click="openExternal"
      >
        <template #icon><ExternalLink :size="14" /></template>
      </VButton>
    </header>

    <div class="notices">
      <VMessage v-if="parseError" severity="error" size="small" data-testid="settings-parse-error">
        {{ t('settings.parseError') }} {{ describeFailure(parseError) }}
      </VMessage>
      <VMessage v-if="issues.length" severity="warn" size="small" data-testid="settings-issues">{{ t('settings.issues') }} {{ issues.join('; ') }}</VMessage>
      <VMessage v-if="ioError" severity="error" size="small" data-testid="settings-io-error">{{ ioError }}</VMessage>
    </div>

    <div class="body">
      <nav class="categories" :aria-label="t('settings.categoriesLabel')">
        <button
          v-for="entry in CATEGORIES"
          :key="entry.id"
          class="category"
          :data-active="!rawMode && !normalizedQuery && entry.id === category"
          :data-testid="`settings-category-${entry.id}`"
          @click="pickCategory(entry.id)"
        >
          <component :is="CATEGORY_ICONS[entry.id]" :size="14" />
          {{ t(entry.label) }}
        </button>
      </nav>

      <div class="content vt-detail">
        <SettingsJsonEditor v-if="rawMode" name="settings.json" :text="text" :save="saveText" />
        <KeymapPanel v-else-if="showKeymap" />
        <div v-else class="list" :class="{ locked: parseError }" data-testid="settings-list">
          <section v-if="showConnectors" class="group" data-testid="settings-connectors">
            <ConnectorsPanel />
          </section>
          <section v-if="showRemote" class="group" data-testid="settings-remote">
            <RemotePanel />
          </section>
          <section v-for="group in groups" :key="group.id" v-auto-animate class="group">
            <h2 v-if="normalizedQuery">{{ t(group.label) }}</h2>
            <SettingRow v-for="key in group.keys" :key="key" :setting-key="key" />
          </section>
          <p v-if="!groups.length && !showConnectors" class="empty" data-testid="settings-empty">{{ t('settings.empty') }}</p>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.page {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  color: var(--vr-color-text);
}

.header {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-4) var(--vr-space-6);
  border-bottom: 1px solid var(--vr-color-border);
}

h1 {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0 var(--vr-space-4) 0 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
}

.search {
  position: relative;
  display: flex;
  flex: 1;
  align-items: center;
  max-width: 28rem;
  color: var(--vr-color-text-muted);
}

.search svg {
  position: absolute;
  left: var(--vr-space-2);
  pointer-events: none;
}

.search :deep(input) {
  width: 100%;
  padding-left: calc(var(--vr-space-2) * 3.5);
}

.notices {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-2);
}

.notices:not(:empty) {
  padding: var(--vr-space-3) var(--vr-space-6) 0;
}

.body {
  display: grid;
  flex: 1;
  grid-template-columns: minmax(12rem, max-content) minmax(0, 1fr);
  min-height: 0;
}

.categories {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  padding: var(--vr-space-4) var(--vr-space-3);
  border-right: 1px solid var(--vr-color-border);
}

.category {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-3);
  font: inherit;
  text-align: left;
  white-space: nowrap;
  color: inherit;
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.category:hover {
  background: var(--vr-color-nav-hover);
}

.category[data-active='true'] {
  font-weight: 600;
  background: var(--vr-color-nav-active);
}

.content {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.list {
  flex: 1;
  min-height: 0;
  padding: var(--vr-space-2) var(--vr-space-6) var(--vr-space-6);
  overflow-y: auto;
}

.list.locked {
  opacity: 0.6;
  pointer-events: none;
}

.group h2 {
  margin: var(--vr-space-4) 0 0;
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-muted);
  text-transform: uppercase;
}

.empty {
  margin: var(--vr-space-6) 0;
  color: var(--vr-color-text-muted);
}
</style>
