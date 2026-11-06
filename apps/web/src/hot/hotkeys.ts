import { shallowRef } from 'vue';
import { useDock } from '../workbench/dock';
import { useHot } from './hot';
import { hotEnabled } from './view';

export const switcher = shallowRef<{ keys: readonly string[]; index: number } | null>(null);

const DIGIT = /^Digit([1-9])$/;

function swallow(event: KeyboardEvent): void {
  event.preventDefault();
  event.stopPropagation();
}

function cycle(step: 1 | -1): void {
  const hot = useHot();
  const current = switcher.value;
  if (current) {
    const size = current.keys.length;
    switcher.value = { keys: current.keys, index: (current.index + step + size) % size };
    return;
  }
  const keys = hot.mru.value;
  if (!keys.length) return;
  const active = useDock().activeKey.value;
  const start = keys.length > 1 && keys[0] === active ? 1 : 0;
  switcher.value = { keys, index: step > 0 ? start : keys.length - 1 };
}

function commit(): void {
  const current = switcher.value;
  switcher.value = null;
  const key = current?.keys[current.index];
  if (key) useHot().open(key);
}

export function chooseSwitcher(index: number): void {
  const current = switcher.value;
  if (current) switcher.value = { keys: current.keys, index };
  commit();
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.isComposing || !hotEnabled.value) return;
  if (switcher.value) {
    if (event.code === 'Escape') {
      swallow(event);
      switcher.value = null;
    } else if (event.code === 'Enter') {
      swallow(event);
      commit();
    } else if (event.code === 'Tab' || event.code === 'ArrowDown' || event.code === 'ArrowRight') {
      swallow(event);
      cycle(event.shiftKey && event.code === 'Tab' ? -1 : 1);
    } else if (event.code === 'ArrowUp' || event.code === 'ArrowLeft') {
      swallow(event);
      cycle(-1);
    } else if (!event.ctrlKey) switcher.value = null;
    else swallow(event);
    return;
  }
  if (event.ctrlKey && !event.altKey && !event.metaKey && event.code === 'Tab') {
    swallow(event);
    cycle(event.shiftKey ? -1 : 1);
    return;
  }
  if (event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey && event.code === 'KeyJ') {
    swallow(event);
    useHot().next();
    return;
  }
  const digit = DIGIT.exec(event.code);
  if (digit && event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
    swallow(event);
    const item = useHot().items.value[Number(digit[1]) - 1];
    if (item) useHot().open(item.key);
  }
}

function onKeyUp(event: KeyboardEvent): void {
  if (switcher.value && event.key === 'Control') {
    swallow(event);
    commit();
  }
}

function cancel(): void {
  switcher.value = null;
}

export function installHotkeys(): () => void {
  window.addEventListener('keydown', onKeyDown, true);
  window.addEventListener('keyup', onKeyUp, true);
  window.addEventListener('blur', cancel);
  return () => {
    window.removeEventListener('keydown', onKeyDown, true);
    window.removeEventListener('keyup', onKeyUp, true);
    window.removeEventListener('blur', cancel);
    switcher.value = null;
  };
}
