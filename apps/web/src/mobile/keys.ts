import { shallowRef } from 'vue';

export const ctrlArmed = shallowRef(false);

const CTRL_MASK = 0x1f;

export function applyCtrl(data: string): string {
  if (!ctrlArmed.value || data.length !== 1) return data;
  ctrlArmed.value = false;
  const code = data.toUpperCase().charCodeAt(0);
  return code >= 0x40 && code <= 0x5f ? String.fromCharCode(code & CTRL_MASK) : data;
}
