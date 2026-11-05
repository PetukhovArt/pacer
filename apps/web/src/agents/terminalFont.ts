import { computed } from 'vue';
import { useSettings } from '../settings/store';

const { values, set, reset } = useSettings();

const terminalFontSize = computed(() => values.value['terminal.font_size']);

export function zoomTerminal(step: -1 | 0 | 1): void {
  if (step === 0) reset('terminal.font_size');
  else set('terminal.font_size', terminalFontSize.value + step);
}

export function zoomStep(event: KeyboardEvent): -1 | 0 | 1 | null {
  if (!event.ctrlKey || event.altKey || event.metaKey) return null;
  if (event.code === 'Equal' || event.code === 'NumpadAdd') return 1;
  if (event.code === 'Minus' || event.code === 'NumpadSubtract') return -1;
  if (event.code === 'Digit0' || event.code === 'Numpad0') return 0;
  return null;
}
