<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { VButton, VInputText } from '@ui/kit';
import { describeError, jiraSearch, reloadWhenRecovered, type JiraIssueSummary } from '../connectors/api';
import { t, useI18n } from '../i18n';
import ErrorInline from './ErrorInline.vue';
import PersonAvatar from './PersonAvatar.vue';
import PriorityMark from './PriorityMark.vue';
import Skeleton from './Skeleton.vue';
import StatusChip from './StatusChip.vue';
import TypeIcon from './TypeIcon.vue';
import { readStored, writeStored } from './util';

type Filter = { id: string; name: string; jql: string };

const props = defineProps<{ connector: string }>();
const emit = defineEmits<{ open: [payload: { connector: string; key: string }] }>();

const { formatRelative, formatDate } = useI18n();

const PAGE = 50;
const FILTERS_KEY = 'veyra.jira.filters';
const HISTORY_KEY = 'veyra.jira.history';

const PRESETS: Omit<Filter, 'name'>[] = [
  { id: 'mine', jql: 'assignee = currentUser() AND resolution = Unresolved ORDER BY updated DESC' },
  { id: 'reported', jql: 'reporter = currentUser() ORDER BY created DESC' },
  { id: 'recent', jql: 'updated >= -7d ORDER BY updated DESC' },
  { id: 'sprint', jql: 'sprint in openSprints() AND assignee = currentUser()' },
];

const PRESET_NAMES: Record<string, () => string> = {
  mine: () => t('jira.list.presets.mine'),
  reported: () => t('jira.list.presets.reported'),
  recent: () => t('jira.list.presets.recent'),
  sprint: () => t('jira.list.presets.sprint'),
};

function presetName(id: string): string {
  return PRESET_NAMES[id]?.() ?? id;
}

const vFocus = { mounted: (el: HTMLElement) => el.focus() };

const custom = ref<Filter[]>(readStored<Filter>(FILTERS_KEY));
watch(custom, (value) => writeStored(FILTERS_KEY, value), { deep: true });
const history = ref<string[]>(readStored<string>(HISTORY_KEY));

const activeId = ref<string | null>(PRESETS[0]!.id);
const draft = ref(PRESETS[0]!.jql);
const current = ref(PRESETS[0]!.jql);
const issues = ref<JiraIssueSummary[]>([]);
const total = ref(0);
const loading = ref(false);
const loadingMore = ref(false);
const error = ref('');
const cursor = ref(-1);
const editing = ref<{ id: string; name: string } | null>(null);
const searchFocused = ref(false);
const typed = ref(false);
const suggestionCursor = ref(-1);
const root = ref<HTMLElement | null>(null);
const search = ref<{ $el: HTMLInputElement } | null>(null);
let seq = 0;

const allFilters = computed(() => [...PRESETS, ...custom.value]);

const suggestions = computed(() => {
  if (!searchFocused.value) return [];
  const needle = draft.value.trim().toLowerCase();
  return history.value.filter((jql) => jql !== draft.value && (!typed.value || jql.toLowerCase().includes(needle))).slice(0, 8);
});

async function load(more: boolean) {
  const id = ++seq;
  if (more) {
    loadingMore.value = true;
  } else {
    loading.value = true;
    issues.value = [];
    total.value = 0;
    cursor.value = -1;
  }
  error.value = '';
  try {
    const result = await jiraSearch(props.connector, current.value, more ? issues.value.length : 0, PAGE);
    if (id !== seq) return;
    issues.value = more ? [...issues.value, ...result.issues] : result.issues;
    total.value = result.total;
  } catch (e) {
    if (id === seq) error.value = describeError(e);
  } finally {
    if (id === seq) {
      loading.value = false;
      loadingMore.value = false;
    }
  }
}

function run(jql: string, id: string | null) {
  current.value = jql;
  draft.value = jql;
  activeId.value = id;
  void load(false);
}

function applyFilter(filter: Pick<Filter, 'id' | 'jql'>) {
  run(filter.jql, filter.id);
}

function remember(jql: string) {
  history.value = [jql, ...history.value.filter((item) => item !== jql)].slice(0, 20);
  writeStored(HISTORY_KEY, history.value);
}

function submit() {
  const jql = draft.value.trim();
  if (!jql) return;
  remember(jql);
  run(jql, allFilters.value.find((filter) => filter.jql === jql)?.id ?? null);
}

function pickSuggestion(jql: string) {
  suggestionCursor.value = -1;
  draft.value = jql;
  submit();
}

function onSearchKey(event: KeyboardEvent) {
  const list = suggestions.value;
  if (event.key === 'Enter') {
    event.preventDefault();
    const picked = list[suggestionCursor.value];
    if (picked) pickSuggestion(picked);
    else submit();
  } else if (event.key === 'ArrowDown') {
    event.preventDefault();
    if (list.length) suggestionCursor.value = Math.min(suggestionCursor.value + 1, list.length - 1);
    else leaveSearch();
  } else if (event.key === 'ArrowUp' && list.length) {
    event.preventDefault();
    suggestionCursor.value = Math.max(suggestionCursor.value - 1, -1);
  } else if (event.key === 'Escape') {
    leaveSearch(false);
  }
}

function leaveSearch(selectRow = true) {
  search.value?.$el.blur();
  root.value?.focus();
  if (selectRow && cursor.value < 0 && issues.value.length) moveCursor(0);
}

function focusSearch() {
  search.value?.$el.focus();
  search.value?.$el.select();
}

function moveCursor(index: number) {
  cursor.value = index;
  void nextTick(() => root.value?.querySelector(`[data-row="${index}"]`)?.scrollIntoView({ block: 'nearest' }));
}

function open(issue: JiraIssueSummary) {
  emit('open', { connector: props.connector, key: issue.key });
}

function onKey(event: KeyboardEvent) {
  const target = event.target as HTMLElement;
  if (target.closest('input, textarea')) return;
  if (event.key === '/') {
    event.preventDefault();
    focusSearch();
  } else if (target.closest('button')) {
    return;
  } else if (event.key === 'ArrowDown') {
    event.preventDefault();
    moveCursor(Math.min(cursor.value + 1, issues.value.length - 1));
  } else if (event.key === 'ArrowUp') {
    event.preventDefault();
    moveCursor(Math.max(cursor.value - 1, 0));
  } else if (event.key === 'Enter') {
    const issue = issues.value[cursor.value];
    if (issue) open(issue);
  }
}

function clickRow(index: number) {
  cursor.value = index;
  open(issues.value[index]!);
}

function startAdd() {
  if (!draft.value.trim()) return;
  editing.value = { id: '', name: '' };
}

function startRename(filter: Filter) {
  editing.value = { id: filter.id, name: filter.name };
}

function commitEdit() {
  const edit = editing.value;
  if (!edit) return;
  editing.value = null;
  const name = edit.name.trim();
  if (!name) return;
  if (edit.id) {
    custom.value = custom.value.map((filter) => (filter.id === edit.id ? { ...filter, name } : filter));
  } else {
    const filter = { id: crypto.randomUUID(), name, jql: draft.value.trim() };
    custom.value = [...custom.value, filter];
    activeId.value = filter.id;
  }
}

function removeFilter(filter: Filter) {
  custom.value = custom.value.filter((item) => item.id !== filter.id);
  if (activeId.value === filter.id) activeId.value = null;
}

watch(
  () => props.connector,
  () => void load(false),
);
reloadWhenRecovered(
  () => props.connector,
  () => error.value !== '',
  () => void load(false),
);

onMounted(() => void load(false));
</script>

<template>
  <div ref="root" class="jira-issues" tabindex="0" data-testid="jira-issues" @keydown="onKey">
    <aside class="filters" :aria-label="t('jira.list.filters')">
      <div class="group-title">{{ t('jira.list.filters') }}</div>
      <button v-for="filter in PRESETS" :key="filter.id" class="chip" :class="{ active: activeId === filter.id }" :title="filter.jql" @click="applyFilter(filter)">
        {{ presetName(filter.id) }}
      </button>
      <div class="group-title">{{ t('jira.list.myFilters') }}</div>
      <div v-for="filter in custom" :key="filter.id" class="chip-row">
        <VInputText
          v-if="editing?.id === filter.id"
          v-model="editing.name"
          v-focus
          size="small"
          class="rename"
          :aria-label="t('jira.list.filterName')"
          @keydown.enter.prevent="commitEdit"
          @keydown.esc.prevent="editing = null"
          @blur="commitEdit"
        />
        <template v-else>
          <button class="chip" :class="{ active: activeId === filter.id }" :title="filter.jql" @click="applyFilter(filter)" @dblclick="startRename(filter)">
            {{ filter.name }}
          </button>
          <button class="mini" :title="t('common.rename')" :aria-label="t('jira.list.renameFilter')" @click="startRename(filter)">✎</button>
          <button class="mini" :title="t('common.delete')" :aria-label="t('jira.list.deleteFilter')" @click="removeFilter(filter)">×</button>
        </template>
      </div>
      <VInputText
        v-if="editing?.id === ''"
        v-model="editing.name"
        v-focus
        size="small"
        class="rename"
        :placeholder="t('jira.list.filterName')"
        :aria-label="t('jira.list.newFilterName')"
        @keydown.enter.prevent="commitEdit"
        @keydown.esc.prevent="editing = null"
        @blur="commitEdit"
      />
      <div v-if="!custom.length && editing?.id !== ''" class="hint">{{ t('jira.list.hint') }}</div>
    </aside>

    <section class="main">
      <div class="toolbar">
        <div class="search">
          <VInputText
            ref="search"
            v-model="draft"
            class="jql"
            :placeholder="t('jira.list.jqlPlaceholder')"
            aria-label="JQL"
            data-testid="jira-jql"
            spellcheck="false"
            @focus="searchFocused = true; typed = false; suggestionCursor = -1"
            @blur="searchFocused = false"
            @input="typed = true; suggestionCursor = -1"
            @keydown="onSearchKey"
          />
          <ul v-if="suggestions.length" class="suggestions" role="listbox">
            <li
              v-for="(jql, index) in suggestions"
              :key="jql"
              role="option"
              :aria-selected="index === suggestionCursor"
              :class="{ active: index === suggestionCursor }"
              @mousedown.prevent
              @click="pickSuggestion(jql)"
            >
              {{ jql }}
            </li>
          </ul>
        </div>
        <VButton :label="t('jira.list.find')" size="small" :disabled="!draft.trim()" @click="submit" />
        <VButton :label="t('jira.list.saveFilter')" size="small" severity="secondary" :disabled="!draft.trim()" @click="startAdd" />
      </div>

      <div class="body">
        <ErrorInline v-if="error" :message="error" @retry="load(issues.length > 0)" />
        <Skeleton v-if="loading" :rows="10" />
        <div v-else-if="!issues.length && !error" class="empty">{{ t('jira.list.empty') }}</div>
        <table v-else-if="issues.length" class="table">
          <colgroup>
            <col class="c-type" />
            <col class="c-key" />
            <col />
            <col class="c-status" />
            <col class="c-priority" />
            <col class="c-assignee" />
            <col class="c-updated" />
          </colgroup>
          <thead>
            <tr>
              <th :aria-label="t('jira.list.columns.type')" />
              <th>{{ t('jira.list.columns.key') }}</th>
              <th>{{ t('jira.list.columns.summary') }}</th>
              <th>{{ t('jira.list.columns.status') }}</th>
              <th>{{ t('jira.list.columns.priority') }}</th>
              <th>{{ t('jira.list.columns.assignee') }}</th>
              <th>{{ t('jira.list.columns.updated') }}</th>
            </tr>
          </thead>
          <tbody v-auto-animate>
            <tr v-for="(issue, index) in issues" :key="issue.key" :data-row="index" :class="{ cursor: index === cursor }" data-testid="jira-row" @click="clickRow(index)">
              <td><TypeIcon :type="issue.issue_type" /></td>
              <td class="key">{{ issue.key }}</td>
              <td class="summary" :title="issue.summary">{{ issue.summary }}</td>
              <td><StatusChip :status="issue.status" :category="issue.status_category" /></td>
              <td><PriorityMark :priority="issue.priority" /></td>
              <td>
                <PersonAvatar v-if="issue.assignee" :connector="issue.connector" :person="issue.assignee" small />
                <span v-else class="muted">-</span>
              </td>
              <td class="muted" :title="formatDate(issue.updated)">{{ formatRelative(issue.updated) }}</td>
            </tr>
          </tbody>
        </table>
        <div v-if="issues.length && !loading" class="more">
          <span class="muted">{{ t('jira.list.shown', { shown: issues.length, total }) }}</span>
          <VButton v-if="issues.length < total" :label="t('jira.list.loadMore')" size="small" severity="secondary" :loading="loadingMore" @click="load(true)" />
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.jira-issues {
  display: flex;
  width: 100%;
  height: 100%;
  min-height: 0;
  background: var(--vr-color-surface);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  outline: none;
}

.filters {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--vr-space-1);
  width: 210px;
  padding: var(--vr-space-3);
  overflow: auto;
  border-right: 1px solid var(--vr-color-border);
}

.group-title {
  margin-top: var(--vr-space-2);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  text-transform: uppercase;
}

.chip-row {
  display: flex;
  align-items: center;
  gap: 2px;
}

.chip {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  padding: var(--vr-space-1) var(--vr-space-3);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-full);
  background: transparent;
  color: var(--vr-color-text);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.chip:hover {
  background: var(--vr-color-nav-hover);
}

.chip.active {
  border-color: var(--vr-color-accent);
  background: var(--vr-color-chip-bg);
}

.mini {
  flex: none;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: var(--vr-radius-sm);
  background: transparent;
  color: var(--vr-color-text-muted);
  opacity: 0;
  cursor: pointer;
}

.chip-row:hover .mini,
.chip-row:focus-within .mini {
  opacity: 1;
}

.mini:hover {
  background: var(--vr-color-nav-hover);
  color: var(--vr-color-text);
}

.rename {
  width: 100%;
}

.hint {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.toolbar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-2);
  padding: var(--vr-space-3);
  border-bottom: 1px solid var(--vr-color-border);
}

.search {
  position: relative;
  flex: 1;
  min-width: 0;
}

.jql {
  width: 100%;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
}

.suggestions {
  position: absolute;
  z-index: 10;
  top: 100%;
  right: 0;
  left: 0;
  margin: var(--vr-space-1) 0 0;
  padding: var(--vr-space-1);
  list-style: none;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
  background: var(--vr-color-surface-raised);
  box-shadow: var(--vr-shadow-popover);
}

.suggestions li {
  overflow: hidden;
  padding: var(--vr-space-1) var(--vr-space-2);
  border-radius: var(--vr-radius-sm);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.suggestions li:hover,
.suggestions li.active {
  background: var(--vr-color-surface-muted);
}

.body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.empty {
  padding: var(--vr-space-6);
  color: var(--vr-color-text-muted);
  text-align: center;
}

.table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}

.c-type { width: 36px; }
.c-key { width: 100px; }
.c-status { width: 140px; }
.c-priority { width: 110px; }
.c-assignee { width: 56px; }
.c-updated { width: 120px; }

.table th {
  position: sticky;
  z-index: 1;
  top: 0;
  padding: var(--vr-space-2);
  border-bottom: 1px solid var(--vr-color-border);
  background: var(--vr-color-surface);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-2xs);
  font-weight: 600;
  text-align: left;
  text-transform: uppercase;
}

.table td {
  height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  overflow: hidden;
  border-bottom: 1px solid var(--vr-color-nav-hover);
  font-size: var(--vr-font-size-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.table tbody tr {
  cursor: pointer;
}

.table tbody tr:hover {
  background: var(--vr-color-nav-hover);
}

.table tbody tr.cursor {
  background: var(--vr-color-nav-active);
}

.key {
  font-family: var(--vr-font-mono);
  color: var(--vr-color-text-muted);
}

.summary {
  font-size: var(--vr-font-size-sm);
}

.muted {
  color: var(--vr-color-text-muted);
}

.more {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--vr-space-3);
  font-size: var(--vr-font-size-xs);
}
</style>
