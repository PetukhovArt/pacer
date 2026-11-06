import { computed, shallowRef, watchEffect } from 'vue';
import { DARK_CLASS } from '@ui/kit';
import { SETTINGS } from './schema';
import { useSettings } from './store';

const SCALED_TOKENS = ['font-size-2xs', 'font-size-xs', 'font-size-sm', 'font-size-base', 'font-size-lg', 'font-size-xl', 'font-leading-message', 'font-leading-author'];
const BASE_ROOT_PX = 16;

const { values, set } = useSettings();
const scheme = window.matchMedia('(prefers-color-scheme: dark)');
const systemDark = shallowRef(scheme.matches);
scheme.addEventListener('change', (event) => {
  systemDark.value = event.matches;
});

export const isDark = computed(() => (values.value.theme === 'system' ? systemDark.value : values.value.theme === 'dark'));

export function toggleTheme(): void {
  set('theme', isDark.value ? 'light' : 'dark');
}

export function installAppearance(): void {
  const root = document.documentElement;
  const computedStyle = getComputedStyle(root);
  const baseTokens = SCALED_TOKENS.map((name) => [`--vr-${name}`, parseFloat(computedStyle.getPropertyValue(`--vr-${name}`))] as const);

  watchEffect(() => root.classList.toggle(DARK_CLASS, isDark.value));
  watchEffect(() => {
    const scale = values.value['ui.font_size'] / SETTINGS['ui.font_size'].default;
    if (scale === 1) {
      root.style.removeProperty('font-size');
      for (const [name] of baseTokens) root.style.removeProperty(name);
      return;
    }
    root.style.fontSize = `${BASE_ROOT_PX * scale}px`;
    for (const [name, base] of baseTokens) root.style.setProperty(name, `${Math.round(base * scale * 10) / 10}px`);
  });
}
