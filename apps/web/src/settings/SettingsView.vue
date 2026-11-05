<script setup lang="ts">
import { computed, shallowRef, watch, type Component } from 'vue';
import { ExternalLink, FileJson, History, Palette, Plug, Search, SlidersHorizontal, SquareTerminal } from 'lucide-vue-next';
import { VButton, VInputText, VMessage } from '@ui/kit';
import ConnectorsPanel from '../connectors/ConnectorsPanel.vue';
import { requestedSettingsCategory } from '../nav';
import SettingRow from './SettingRow.vue';
import SettingsJsonEditor from './SettingsJsonEditor.vue';
import { CATEGORIES, SETTINGS, SETTING_KEYS } from './schema';
import { useSettings } from './store';
import { describeFailure } from './parse';

const { parseError, issues, ioError, openExternal, canOpenExternal } = useSettings();

const query = shallowRef('');
type CategoryId = (typeof CATEGORIES)[number]['id'];
const category = shallowRef<CategoryId>('appearance');
const rawMode = shallowRef(false);

const CATEGORY_ICONS: Record<CategoryId, Component> = { appearance: Palette, terminal: SquareTerminal, sessions: History, connectors: Plug };
const CONNECTOR_KEYWORDS = 'коннекторы коннектор github gitlab jira confluence токен токены подключение аккаунт';

const normalizedQuery = computed(() => query.value.trim().toLowerCase());
watch(normalizedQuery, (value) => {
  if (value) rawMode.value = false;
});
const showConnectors = computed(() => {
  if (!normalizedQuery.value) return category.value === 'connectors';
  return normalizedQuery.value.split(/\s+/).every((word) => CONNECTOR_KEYWORDS.includes(word));
});
const groups = computed(() =>
  CATEGORIES.map((entry) => ({
    ...entry,
    keys: SETTING_KEYS.filter((key) => {
      const def = SETTINGS[key];
      if (def.category !== entry.id) return false;
      if (!normalizedQuery.value) return entry.id === category.value;
      return [def.label, key, def.description].some((field) => field.toLowerCase().includes(normalizedQuery.value));
    }),
  })).filter((group) => group.keys.length),
);

function pickCategory(id: CategoryId) {
  category.value = id;
  query.value = '';
  rawMode.value = false;
}

watch(
  requestedSettingsCategory,
  (id) => {
    if (!id) return;
    requestedSettingsCategory.value = null;
    const entry = CATEGORIES.find((item) => item.id === id);
    if (entry) pickCategory(entry.id);
  },
  { immediate: true },
);

function onSearchKey(event: KeyboardEvent) {
  if (event.key === 'Escape') query.value = '';
}
</script>

<template>
  <section class="page" data-testid="settings-view" aria-label="Настройки">
    <header class="header">
      <h1><SlidersHorizontal :size="18" />Настройки</h1>
      <label class="search">
        <Search :size="16" />
        <VInputText v-model="query" size="small" placeholder="Найти настройку" aria-label="Найти настройку" data-testid="settings-search" @keydown="onSearchKey" />
      </label>
      <VButton
        :label="rawMode ? 'К настройкам' : 'Открыть settings.json'"
        size="small"
        severity="secondary"
        data-testid="settings-open-json"
        @click="rawMode = !rawMode"
      >
        <template #icon><FileJson :size="14" /></template>
      </VButton>
      <VButton
        label="Открыть в редакторе"
        size="small"
        severity="secondary"
        text
        :disabled="!canOpenExternal"
        :title="canOpenExternal ? 'Открыть файл в редакторе по умолчанию' : 'Доступно только в приложении'"
        data-testid="settings-open-external"
        @click="openExternal"
      >
        <template #icon><ExternalLink :size="14" /></template>
      </VButton>
    </header>

    <div class="notices">
      <VMessage v-if="parseError" severity="error" size="small" data-testid="settings-parse-error">
        settings.json не разобран, действуют последние верные значения. {{ describeFailure(parseError) }}
      </VMessage>
      <VMessage v-if="issues.length" severity="warn" size="small" data-testid="settings-issues">Для этих ключей взято значение по умолчанию: {{ issues.join('; ') }}</VMessage>
      <VMessage v-if="ioError" severity="error" size="small" data-testid="settings-io-error">{{ ioError }}</VMessage>
    </div>

    <div class="body">
      <nav class="categories" aria-label="Категории настроек">
        <button
          v-for="entry in CATEGORIES"
          :key="entry.id"
          class="category"
          :data-active="!rawMode && !normalizedQuery && entry.id === category"
          :data-testid="`settings-category-${entry.id}`"
          @click="pickCategory(entry.id)"
        >
          <component :is="CATEGORY_ICONS[entry.id]" :size="14" />
          {{ entry.label }}
        </button>
      </nav>

      <SettingsJsonEditor v-if="rawMode" />
      <div v-else class="list" :class="{ locked: parseError }" data-testid="settings-list">
        <section v-if="showConnectors" class="group" data-testid="settings-connectors">
          <ConnectorsPanel />
        </section>
        <section v-for="group in groups" :key="group.id" class="group">
          <h2 v-if="normalizedQuery">{{ group.label }}</h2>
          <SettingRow v-for="key in group.keys" :key="key" :setting-key="key" />
        </section>
        <p v-if="!groups.length && !showConnectors" class="empty" data-testid="settings-empty">Ничего не найдено</p>
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
  grid-template-columns: 12rem minmax(0, 1fr);
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

.list {
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
