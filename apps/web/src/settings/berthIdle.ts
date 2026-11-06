import { watch } from 'vue';
import { hasBackend, invoke } from '../bridge';
import { SETTINGS } from './schema';
import { useSettings } from './store';

const KEY = 'sessions.unloadIdleAfter';
const CHOICES = new Set<string>(SETTINGS[KEY].options.map((option) => option.value));

export function installBerthIdleSync(): void {
  if (!hasBackend) return;
  const { loaded, values, set } = useSettings();
  let known: string | null = null;

  async function pull(): Promise<void> {
    try {
      const remote = await invoke<string>('berth_idle_timeout_read');
      if (!CHOICES.has(remote)) return;
      known = remote;
      if (values.value[KEY] !== remote) set(KEY, remote);
    } catch {
      return;
    }
  }

  watch(
    () => values.value[KEY],
    async (next) => {
      if (known === null || next === known) return;
      try {
        await invoke<void>('berth_idle_timeout_write', { value: next });
        known = next;
      } catch {
        return;
      }
    },
  );

  let started = false;
  watch(
    loaded,
    (ready) => {
      if (!ready || started) return;
      started = true;
      void pull();
      window.addEventListener('focus', () => void pull());
    },
    { immediate: true },
  );
}
