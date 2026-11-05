import { computed, shallowRef, type ComputedRef, type ShallowRef } from 'vue';
import { describeError, forgeInbox, type InboxError, type InboxItem } from '../connectors/api';

const REFRESH_MS = 2 * 60_000;
const TICK_MS = 30_000;

type InboxStore = {
  items: ShallowRef<InboxItem[]>;
  errors: ShallowRef<InboxError[]>;
  failure: ShallowRef<string | null>;
  loading: ShallowRef<boolean>;
  loadedOnce: ShallowRef<boolean>;
  loadedAt: ShallowRef<number>;
  reviewCount: ComputedRef<number>;
  load: () => Promise<void>;
  loadIfStale: (maxAgeMs: number) => void;
  startPolling: () => () => void;
};

let store: InboxStore | null = null;

function createStore(): InboxStore {
  const items = shallowRef<InboxItem[]>([]);
  const errors = shallowRef<InboxError[]>([]);
  const failure = shallowRef<string | null>(null);
  const loading = shallowRef(false);
  const loadedOnce = shallowRef(false);
  const loadedAt = shallowRef(0);
  const reviewCount = computed(() => items.value.filter((item) => item.reasons.includes('reviewer')).length);

  async function load() {
    if (loading.value) return;
    loading.value = true;
    try {
      const inbox = await forgeInbox();
      items.value = inbox.items;
      errors.value = inbox.errors;
      failure.value = null;
    } catch (e) {
      failure.value = describeError(e);
    } finally {
      loading.value = false;
      loadedOnce.value = true;
      loadedAt.value = Date.now();
    }
  }

  function loadIfStale(maxAgeMs: number) {
    if (Date.now() - loadedAt.value >= maxAgeMs) void load();
  }

  let pollers = 0;
  let timer: ReturnType<typeof setInterval> | undefined;

  function tick() {
    if (document.visibilityState === 'visible') loadIfStale(REFRESH_MS);
  }

  function startPolling() {
    if (pollers++ === 0) {
      tick();
      timer = setInterval(tick, TICK_MS);
      document.addEventListener('visibilitychange', tick);
    }
    let stopped = false;
    return () => {
      if (stopped) return;
      stopped = true;
      if (--pollers === 0) {
        clearInterval(timer);
        document.removeEventListener('visibilitychange', tick);
      }
    };
  }

  return { items, errors, failure, loading, loadedOnce, loadedAt, reviewCount, load, loadIfStale, startPolling };
}

export function useForgeInbox(): InboxStore {
  store ??= createStore();
  return store;
}
