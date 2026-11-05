import { shallowRef } from 'vue';
import { view } from '../nav';
import type { Context } from './when';

export type KeyContext = {
  terminalFocus: boolean;
  inputFocus: boolean;
  paletteOpen: boolean;
  view: string;
};

export const paletteOpen = shallowRef(false);

const INPUT_TAGS = new Set(['INPUT', 'TEXTAREA', 'SELECT']);

export function readContext(target: EventTarget | null): KeyContext & Context {
  const element = target instanceof Element ? target : document.activeElement;
  const terminalFocus = Boolean(element?.closest('.xterm'));
  const inputFocus = !terminalFocus && element instanceof HTMLElement && (INPUT_TAGS.has(element.tagName) || element.isContentEditable);
  return { terminalFocus, inputFocus, paletteOpen: paletteOpen.value, view: view.value.kind };
}
