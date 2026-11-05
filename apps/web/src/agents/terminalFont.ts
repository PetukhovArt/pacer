import { shallowRef, watch } from 'vue';

const KEY = 'veyra.terminal.fontSize';
const DEFAULT_SIZE = 13;
const MIN_SIZE = 8;
const MAX_SIZE = 32;

const clamp = (size: number) => Math.min(MAX_SIZE, Math.max(MIN_SIZE, Math.round(size)));

function stored(): number {
  const value = Number(localStorage.getItem(KEY));
  return Number.isFinite(value) && value > 0 ? clamp(value) : DEFAULT_SIZE;
}

export const terminalFontSize = shallowRef(stored());

watch(terminalFontSize, (size) => localStorage.setItem(KEY, String(size)));

export function zoomTerminal(step: -1 | 0 | 1): void {
  terminalFontSize.value = step === 0 ? DEFAULT_SIZE : clamp(terminalFontSize.value + step);
}

export function zoomStep(event: KeyboardEvent): -1 | 0 | 1 | null {
  if (!event.ctrlKey || event.altKey || event.metaKey) return null;
  if (event.code === 'Equal' || event.code === 'NumpadAdd') return 1;
  if (event.code === 'Minus' || event.code === 'NumpadSubtract') return -1;
  if (event.code === 'Digit0' || event.code === 'Numpad0') return 0;
  return null;
}
