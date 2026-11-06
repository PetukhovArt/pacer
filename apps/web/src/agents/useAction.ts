import { shallowRef, type ShallowRef } from 'vue';
import { describeError } from './berth';

type Action = {
  error: ShallowRef<string | null>;
  busy: ShallowRef<boolean>;
  run: (operation: () => Promise<void>) => Promise<boolean>;
  clear: () => void;
};

export function useAction(): Action {
  const error = shallowRef<string | null>(null);
  const busy = shallowRef(false);

  async function run(operation: () => Promise<void>): Promise<boolean> {
    if (busy.value) return false;
    busy.value = true;
    error.value = null;
    try {
      await operation();
      return true;
    } catch (e) {
      error.value = describeError(e);
      return false;
    } finally {
      busy.value = false;
    }
  }

  function clear() {
    error.value = null;
  }

  return { error, busy, run, clear };
}
