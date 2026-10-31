import type { App } from 'vue';
import PrimeVue from 'primevue/config';
import Aura from '@primeuix/themes/aura';
import { definePreset } from '@primeuix/themes';
import { accentScale, surfaceScale } from '@ui/tokens';
import '@ui/tokens/light.css';
import '@ui/tokens/dark.css';

export const DARK_CLASS = 'vr-dark';

const VeyraPreset = definePreset(Aura, {
  semantic: {
    primary: accentScale,
    colorScheme: { light: { surface: surfaceScale }, dark: { surface: surfaceScale } },
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
