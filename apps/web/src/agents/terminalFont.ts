import { computed } from 'vue';
import { useSettings } from '../settings/store';

const { values, set, reset } = useSettings();

const terminalFontSize = computed(() => values.value['terminal.font_size']);

export function zoomTerminal(step: -1 | 0 | 1): void {
  if (step === 0) reset('terminal.font_size');
  else set('terminal.font_size', terminalFontSize.value + step);
}
