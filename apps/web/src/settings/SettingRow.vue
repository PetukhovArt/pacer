<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { RotateCcw } from 'lucide-vue-next';
import { VInputText, VSelect, VToggleSwitch } from '@ui/kit';
import { useI18n } from '../i18n';
import LanguageSwitch from '../i18n/LanguageSwitch.vue';
import { SETTINGS, type SettingKey } from './schema';
import { useSettings } from './store';

const props = defineProps<{ settingKey: SettingKey }>();

const { t } = useI18n();
const { values, set, reset, isModified } = useSettings();
const def = computed(() => SETTINGS[props.settingKey]);
const value = computed(() => values.value[props.settingKey]);
const modified = computed(() => isModified(props.settingKey));
const display = (next: unknown) =>
  typeof next === 'object' && next !== null
    ? Object.entries(next)
        .map(([key, item]) => `${key}=${String(item)}`)
        .join(', ')
    : String(next);
const draft = shallowRef(display(value.value));

function parseMap(text: string): Record<string, string> {
  const pairs = text.split(/[,;\n]/).flatMap((part) => {
    const at = part.indexOf('=');
    const key = part.slice(0, at).trim();
    const item = part.slice(at + 1).trim();
    return at > 0 && key && item ? [[key, item] as const] : [];
  });
  return Object.fromEntries(pairs);
}
const placeholder = computed(() => ('placeholder' in def.value && def.value.placeholder ? t(def.value.placeholder) : undefined));
const testId = computed(() => `setting-${props.settingKey}`);
const options = computed(() => (def.value.type === 'enum' ? def.value.options.map((option) => ({ value: option.value, label: t(option.label) })) : []));

watch(value, (next) => {
  draft.value = display(next);
});

function commit() {
  const parsed = def.value.type === 'number' ? (draft.value.trim() === '' ? NaN : Number(draft.value)) : def.value.type === 'map' ? parseMap(draft.value) : draft.value;
  if (typeof parsed === 'number' && Number.isNaN(parsed)) {
    draft.value = display(value.value);
    return;
  }
  set(props.settingKey, parsed);
  draft.value = display(values.value[props.settingKey]);
}
</script>

<template>
  <div class="row" :data-modified="modified" :data-testid="`setting-row-${settingKey}`">
    <div class="meta">
      <label class="label" :for="testId">{{ t(def.label) }}</label>
      <p class="description">{{ t(def.description) }}</p>
      <code class="key">{{ settingKey }}</code>
    </div>
    <div class="control">
      <VToggleSwitch v-if="def.type === 'boolean'" :input-id="testId" :model-value="value === true" :data-testid="testId" @update:model-value="set(settingKey, $event)" />
      <div v-else-if="settingKey === 'ui.language'"><LanguageSwitch /></div>
      <VSelect
        v-else-if="def.type === 'enum'"
        :model-value="value"
        :options="options"
        option-label="label"
        option-value="value"
        size="small"
        :input-id="testId"
        :aria-label="t(def.label)"
        :data-testid="testId"
        @update:model-value="set(settingKey, $event)"
      />
      <VInputText
        v-else-if="def.type === 'number'"
        v-model="draft"
        type="number"
        size="small"
        :min="def.min"
        :max="def.max"
        :step="def.step"
        :id="testId"
        :data-testid="testId"
        @change="commit"
      />
      <VInputText v-else v-model="draft" size="small" :placeholder="placeholder" :id="testId" :data-testid="testId" @change="commit" />
      <button v-if="modified" class="reset" :title="t('settings.resetTitle')" :aria-label="t('settings.resetLabel')" :data-testid="`setting-reset-${settingKey}`" @click="reset(settingKey)">
        <RotateCcw :size="14" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 18rem);
  align-items: start;
  gap: var(--vr-space-6);
  padding: var(--vr-space-4) 0;
  border-bottom: 1px solid var(--vr-color-border);
}

.meta {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
}

.label {
  font-weight: 600;
}

.description {
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.key {
  align-self: flex-start;
  font-family: var(--vr-font-mono);
  font-size: var(--vr-font-size-2xs);
  color: var(--vr-color-text-muted);
}

.control {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-width: 0;
}

.control > :first-child {
  flex: 1;
  min-width: 0;
}

.control > .p-toggleswitch {
  flex: none;
}

.reset {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-md);
}

.reset:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}
</style>
