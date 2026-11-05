<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef } from 'vue';
import { ExternalLink, FileJson, Search } from 'lucide-vue-next';
import { VButton, VInputText, VMessage } from '@ui/kit';
import { captureChords } from '../commands/dispatcher';
import { sequenceLabel, type Chord } from '../commands/keys';
import {
  canOpenKeymapExternal,
  conflictsFor,
  isModified,
  keymapIoError,
  keymapIssues,
  keymapParseError,
  keymapText,
  openKeymapExternal,
  rebind,
  resetAction,
  saveKeymapText,
  shortcutsOf,
} from '../commands/keymap';
import { getAction, listActions } from '../commands/registry';
import { fuzzyScore } from '../commands/fuzzy';
import type { PlainKey } from './translate';
import { useI18n } from '../i18n';
import { withViewTransition } from '../motion';
import { describeFailure } from './parse';
import SettingsJsonEditor from './SettingsJsonEditor.vue';

const RECORD_IDLE_MS = 700;

const { t } = useI18n();

type Recording = { actionId: string; chords: Chord[]; conflicts: string[] };

const query = shallowRef('');
const jsonMode = shallowRef(false);
const recording = shallowRef<Recording | null>(null);
let stopCapture = () => {};
let idleTimer: ReturnType<typeof setTimeout> | undefined;

const groups = computed(() => {
  const text = query.value.trim();
  const matches = listActions().filter((action) => {
    if (!text) return true;
    const shortcuts = shortcutsOf(action.id)
      .map((shortcut) => shortcut.label)
      .join(' ');
    return fuzzyScore(text, `${t(action.category)} ${t(action.title)} ${action.id} ${shortcuts}`) !== null;
  });
  const byCategory = new Map<PlainKey, typeof matches>();
  for (const action of matches) byCategory.set(action.category, [...(byCategory.get(action.category) ?? []), action]);
  return [...byCategory].map(([category, actions]) => ({ category, actions }));
});

const titleOf = (id: string) => {
  const action = getAction(id);
  return action ? t(action.title) : id;
};
const hintOf = (context: string | null) => {
  if (context === null) return '';
  if (context === 'terminalFocus') return t('settings.keymap.contextTerminal');
  if (context === '!terminalFocus') return t('settings.keymap.contextOutsideTerminal');
  return context;
};

function toggleJson() {
  void withViewTransition(() => {
    jsonMode.value = !jsonMode.value;
  });
}

function cancel() {
  stopCapture();
  clearTimeout(idleTimer);
  recording.value = null;
}

function finish() {
  stopCapture();
  const current = recording.value;
  if (!current) return;
  const conflicts = conflictsFor(current.actionId, current.chords);
  if (conflicts.length) {
    recording.value = { ...current, conflicts };
    return;
  }
  rebind(current.actionId, current.chords);
  recording.value = null;
}

function startRecording(actionId: string) {
  cancel();
  recording.value = { actionId, chords: [], conflicts: [] };
  stopCapture = captureChords((chord) => {
    const current = recording.value;
    if (!current) return;
    if (chord.code === 'Escape' && !chord.ctrl && !chord.alt && !chord.shift && !chord.meta) {
      cancel();
      return;
    }
    recording.value = { ...current, chords: [...current.chords, chord] };
    clearTimeout(idleTimer);
    idleTimer = setTimeout(finish, RECORD_IDLE_MS);
  });
}

function confirm() {
  const current = recording.value;
  if (!current) return;
  rebind(current.actionId, current.chords);
  recording.value = null;
}

onBeforeUnmount(cancel);
</script>

<template>
  <div class="keymap" data-testid="keymap-panel">
    <div class="toolbar">
      <label class="search">
        <Search :size="16" />
        <VInputText v-model="query" size="small" :placeholder="t('settings.keymap.search')" :aria-label="t('settings.keymap.search')" data-testid="keymap-search" />
      </label>
      <VButton :label="jsonMode ? t('settings.keymap.backToList') : t('settings.keymap.openJson')" size="small" severity="secondary" data-testid="keymap-open-json" @click="toggleJson">
        <template #icon><FileJson :size="14" /></template>
      </VButton>
      <VButton
        :label="t('settings.openExternal')"
        size="small"
        severity="secondary"
        text
        :disabled="!canOpenKeymapExternal"
        :title="canOpenKeymapExternal ? t('settings.openExternalTitle') : t('settings.appOnly')"
        data-testid="keymap-open-external"
        @click="openKeymapExternal"
      >
        <template #icon><ExternalLink :size="14" /></template>
      </VButton>
    </div>

    <div class="notices">
      <VMessage v-if="keymapParseError" severity="error" size="small" data-testid="keymap-parse-error">
        {{ t('settings.keymap.parseError') }} {{ describeFailure(keymapParseError) }}
      </VMessage>
      <VMessage v-if="keymapIssues.length" severity="warn" size="small" data-testid="keymap-issues">{{ t('settings.keymap.skipped') }} {{ keymapIssues.join('; ') }}</VMessage>
      <VMessage v-if="keymapIoError" severity="error" size="small" data-testid="keymap-io-error">{{ keymapIoError }}</VMessage>
    </div>

    <SettingsJsonEditor v-if="jsonMode" name="keymap.json" :text="keymapText" :save="saveKeymapText" empty="[]&#10;" test-prefix="keymap" />
    <div v-else class="list" :class="{ locked: keymapParseError }" data-testid="keymap-list">
      <section v-for="group in groups" :key="group.category" v-auto-animate class="group">
        <h2>{{ t(group.category) }}</h2>
        <div v-for="action in group.actions" :key="action.id" class="row" :data-modified="isModified(action.id)" :data-testid="`keymap-row-${action.id}`">
          <div class="what">
            <span class="title">{{ t(action.title) }}</span>
            <span class="id">{{ action.id }}</span>
          </div>
          <div class="keys" :data-testid="`keymap-shortcuts-${action.id}`">
            <template v-if="recording?.actionId === action.id">
              <span class="recording" data-testid="keymap-recording">
                {{ recording.chords.length ? sequenceLabel(recording.chords) : t('settings.keymap.recording') }}
              </span>
            </template>
            <template v-else>
              <span v-for="shortcut in shortcutsOf(action.id)" :key="shortcut.key + (shortcut.context ?? '')" class="shortcut" :data-origin="shortcut.origin">
                <kbd>{{ shortcut.label }}</kbd>
                <span v-if="shortcut.context" class="hint">{{ hintOf(shortcut.context) }}</span>
              </span>
              <span v-if="!shortcutsOf(action.id).length" class="none">{{ t('settings.keymap.unassigned') }}</span>
            </template>
          </div>
          <div class="buttons">
            <VButton
              :label="t('settings.keymap.edit')"
              size="small"
              severity="secondary"
              text
              :disabled="Boolean(keymapParseError)"
              :data-testid="`keymap-edit-${action.id}`"
              @click="startRecording(action.id)"
            />
            <VButton
              :label="t('settings.keymap.reset')"
              size="small"
              severity="secondary"
              text
              :disabled="!isModified(action.id) || Boolean(keymapParseError)"
              :data-testid="`keymap-reset-${action.id}`"
              @click="resetAction(action.id)"
            />
          </div>
          <div v-if="recording?.actionId === action.id && recording.conflicts.length" class="conflict" data-testid="keymap-conflict">
            <VMessage severity="warn" size="small">
              {{ t('settings.keymap.conflict', { shortcut: sequenceLabel(recording.chords), actions: recording.conflicts.map((id) => `«${titleOf(id)}»`).join(', ') }) }}
            </VMessage>
            <VButton :label="t('settings.keymap.force')" size="small" data-testid="keymap-confirm" @click="confirm" />
            <VButton :label="t('common.cancel')" size="small" severity="secondary" text data-testid="keymap-cancel" @click="cancel" />
          </div>
        </div>
      </section>
      <p v-if="!groups.length" class="empty" data-testid="keymap-empty">{{ t('settings.keymap.empty') }}</p>
    </div>
  </div>
</template>

<style scoped>
.keymap {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) var(--vr-space-6) 0;
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

.list {
  flex: 1;
  min-height: 0;
  padding: var(--vr-space-2) var(--vr-space-6) var(--vr-space-6);
  overflow-y: auto;
}

.list.locked {
  opacity: 0.6;
}

.group h2 {
  margin: var(--vr-space-4) 0 var(--vr-space-1);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  color: var(--vr-color-text-muted);
  text-transform: uppercase;
}

.row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto;
  gap: var(--vr-space-3);
  align-items: center;
  min-height: var(--vr-size-row);
  padding: var(--vr-space-1) 0;
  border-bottom: 1px solid var(--vr-color-border);
}

.what {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.title {
  overflow: hidden;
  color: var(--vr-color-text);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.id {
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.keys {
  display: flex;
  flex-wrap: wrap;
  gap: var(--vr-space-2);
  align-items: center;
}

.shortcut {
  display: inline-flex;
  gap: var(--vr-space-2);
  align-items: baseline;
}

kbd {
  padding: 1px var(--vr-space-2);
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-md);
}

.shortcut[data-origin='user'] kbd {
  border-color: var(--vr-color-text-muted);
}

.hint,
.none {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.recording {
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text);
}

.buttons {
  display: flex;
  gap: var(--vr-space-1);
}

.conflict {
  display: flex;
  grid-column: 1 / -1;
  gap: var(--vr-space-3);
  align-items: center;
}

.empty {
  margin: var(--vr-space-6) 0;
  color: var(--vr-color-text-muted);
}
</style>
