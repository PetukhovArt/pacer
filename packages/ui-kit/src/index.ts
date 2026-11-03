import type { App } from 'vue';
import PrimeVue from 'primevue/config';
import Aura from '@primeuix/themes/aura';
import { definePreset } from '@primeuix/themes';
import { inkScale } from '@ui/tokens';
import '@fontsource-variable/inter/opsz.css';
import '@fontsource/jetbrains-mono/400.css';
import '@ui/tokens/light.css';
import '@ui/tokens/dark.css';

export const DARK_CLASS = 'vr-dark';

const v = (name: string) => `var(--vr-${name})`;

const overlay = { background: v('color-surface-raised'), borderColor: v('color-border'), color: v('color-text'), shadow: v('shadow-popover') };

const scheme = {
  surface: inkScale,
  primary: { color: v('color-accent'), contrastColor: v('color-accent-contrast'), hoverColor: v('color-accent'), activeColor: v('color-accent') },
  highlight: { background: v('color-surface-muted'), focusBackground: v('color-surface-muted'), color: v('color-text'), focusColor: v('color-text') },
  formField: {
    background: v('color-surface'),
    disabledBackground: v('color-surface-muted'),
    filledBackground: v('color-surface-muted'),
    filledFocusBackground: v('color-surface'),
    borderColor: v('color-border'),
    hoverBorderColor: v('color-text-muted'),
    focusBorderColor: v('color-accent'),
    color: v('color-text'),
    placeholderColor: v('color-text-muted'),
    floatLabelColor: v('color-text-muted'),
    iconColor: v('color-text-muted'),
  },
  text: { color: v('color-text'), hoverColor: v('color-text'), mutedColor: v('color-text-muted'), hoverMutedColor: v('color-text') },
  content: { background: v('color-surface'), hoverBackground: v('color-surface-muted'), borderColor: v('color-border'), color: v('color-text'), hoverColor: v('color-text') },
  overlay: { select: overlay, popover: overlay, modal: overlay },
  list: { option: { focusBackground: v('color-surface-muted'), selectedBackground: v('color-surface-muted'), selectedFocusBackground: v('color-surface-muted'), color: v('color-text'), focusColor: v('color-text'), selectedColor: v('color-text'), selectedFocusColor: v('color-text') } },
  navigation: { item: { focusBackground: v('color-surface-muted'), activeBackground: v('color-surface-muted'), color: v('color-text'), focusColor: v('color-text'), activeColor: v('color-text') } },
};

const VeyraPreset = definePreset(Aura, {
  primitive: { borderRadius: { none: '0', xs: '4px', sm: v('radius-sm'), md: v('radius-md'), lg: v('radius-lg'), xl: v('radius-xl') } },
  semantic: {
    primary: inkScale,
    focusRing: { color: v('color-accent') },
    colorScheme: { light: scheme, dark: scheme },
  },
});

export function installUiKit(app: App): void {
  app.use(PrimeVue, { theme: { preset: VeyraPreset, options: { darkModeSelector: `.${DARK_CLASS}` } } });
}

export { default as VButton } from 'primevue/button';
export { default as VInputText } from 'primevue/inputtext';
export { default as VTextarea } from 'primevue/textarea';
export { default as VSelect } from 'primevue/select';
export { default as VTag } from 'primevue/tag';
export { default as VMessage } from 'primevue/message';
export { default as VMultiSelect } from 'primevue/multiselect';
export { default as VToggleSwitch } from 'primevue/toggleswitch';
