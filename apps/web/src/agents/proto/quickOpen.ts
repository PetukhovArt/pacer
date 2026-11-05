import { shallowRef } from 'vue';

const handler = shallowRef<(() => void) | null>(null);

export function provideQuickOpen(open: () => void): () => void {
  handler.value = open;
  return () => {
    if (handler.value === open) handler.value = null;
  };
}

export function quickOpen(): void {
  handler.value?.();
}
