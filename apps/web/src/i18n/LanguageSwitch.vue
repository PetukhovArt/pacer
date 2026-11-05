<script setup lang="ts">
import { useI18n, type Locale } from './index';

const { t, locale, setLocale } = useI18n();

const OPTIONS: { value: Locale; label: string }[] = [
  { value: 'en', label: 'EN' },
  { value: 'ru', label: 'RU' },
];
</script>

<template>
  <div class="lang" role="radiogroup" :aria-label="t('common.language.label')" data-testid="language-switch">
    <button
      v-for="option in OPTIONS"
      :key="option.value"
      class="lang-option"
      :class="{ active: locale === option.value }"
      type="button"
      role="radio"
      :aria-checked="locale === option.value"
      :title="t(`common.language.${option.value}`)"
      :data-testid="`language-${option.value}`"
      @click="setLocale(option.value)"
    >
      {{ option.label }}
    </button>
  </div>
</template>

<style scoped>
.lang {
  display: inline-flex;
  padding: 2px;
  gap: 2px;
  border: 1px solid var(--vr-color-border);
  border-radius: var(--vr-radius-sm);
  background: var(--vr-color-surface-muted);
}

.lang-option {
  padding: 2px var(--vr-space-3);
  border: 0;
  border-radius: calc(var(--vr-radius-sm) - 2px);
  background: transparent;
  color: var(--vr-color-text-muted);
  font: inherit;
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  cursor: pointer;
}

.lang-option:hover {
  background: var(--vr-color-nav-hover);
}

.lang-option.active {
  background: var(--vr-color-accent);
  color: var(--vr-color-accent-contrast);
}
</style>
