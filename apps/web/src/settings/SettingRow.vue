<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { RotateCcw } from 'lucide-vue-next';
import { VInputText, VSelect, VToggleSwitch } from '@ui/kit';
import { SETTINGS, type SettingKey } from './schema';
import { useSettings } from './store';

const props = defineProps<{ settingKey: SettingKey }>();

const { values, set, reset, isModified } = useSettings();
const def = computed(() => SETTINGS[props.settingKey]);
const value = computed(() => values.value[props.settingKey]);
const modified = computed(() => isModified(props.settingKey));
const draft = shallowRef(String(value.value));
const placeholder = computed(() => ('placeholder' in def.value ? def.value.placeholder : undefined));
const testId = computed(() => `setting-${props.settingKey}`);
const options = computed(() => (def.value.type === 'enum' ? def.value.options.map((option) => ({ value: option.value, label: option.label })) : []));

watch(value, (next) => {
  draft.value = String(next);
});

function commit() {
  const parsed = def.value.type === 'number' ? (draft.value.trim() === '' ? NaN : Number(draft.value)) : draft.value;
  if (typeof parsed === 'number' && Number.isNaN(parsed)) {
    draft.value = String(value.value);
    return;
  }
  set(props.settingKey, parsed);
  draft.value = String(values.value[props.settingKey]);
}
</script>

<template>
  <div class="row" :data-modified="modified" :data-testid="`setting-row-${settingKey}`">
    <div class="meta">
      <label class="label" :for="testId">{{ def.label }}</label>
      <p class="description">{{ def.description }}</p>
      <code class="key">{{ settingKey }}</code>
    </div>
    <div class="control">
      <VToggleSwitch v-if="def.type === 'boolean'" :input-id="testId" :model-value="value === true" :data-testid="testId" @update:model-value="set(settingKey, $event)" />
      <VSelect
        v-else-if="def.type === 'enum'"
        :model-value="value"
        :options="options"
        option-label="label"
        option-value="value"
        size="small"
        :input-id="testId"
        :aria-label="def.label"
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
      <button v-if="modified" class="reset" title="Сбросить к значению по умолчанию" aria-label="Сбросить" :data-testid="`setting-reset-${settingKey}`" @click="reset(settingKey)">
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
