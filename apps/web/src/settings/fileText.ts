import { shallowRef, type ShallowRef } from 'vue';
import type { Backend } from './backend';

const ECHO_MEMORY = 8;

const describeIoError = (e: unknown) => (e instanceof Error ? e.message : String(e));

type FileText = {
  text: ShallowRef<string>;
  ioError: ShallowRef<string | null>;
  apply: (next: string) => void;
  save: (next: string) => void;
  start: (afterLoad?: () => void) => void;
  openExternal: () => Promise<void>;
  canOpenExternal: boolean;
};

export function createFileText(backend: Backend, onText: (text: string) => void): FileText {
  const text = shallowRef('');
  const ioError = shallowRef<string | null>(null);
  const written: string[] = [];
  let started = false;
  let pending: string | null = null;
  let flushing = false;

  function apply(next: string): void {
    text.value = next;
    onText(next);
  }

  async function flush(): Promise<void> {
    if (flushing) return;
    flushing = true;
    try {
      while (pending !== null) {
        const next = pending;
        pending = null;
        await backend.write(next);
      }
      ioError.value = null;
    } catch (e) {
      ioError.value = describeIoError(e);
    } finally {
      flushing = false;
    }
  }

  function persist(next: string): void {
    written.push(next);
    if (written.length > ECHO_MEMORY) written.shift();
    pending = next;
    void flush();
  }

  function save(next: string): void {
    apply(next);
    persist(next);
  }

  function onDisk(next: string): void {
    if (next === text.value) {
      written.length = 0;
      return;
    }
    if (written.includes(next)) return;
    written.length = 0;
    apply(next);
  }

  async function load(afterLoad?: () => void): Promise<void> {
    try {
      const disk = await backend.read();
      if (disk !== text.value) apply(disk);
    } catch (e) {
      ioError.value = describeIoError(e);
      return;
    }
    afterLoad?.();
    backend.watch(onDisk);
  }

  function start(afterLoad?: () => void): void {
    if (started) return;
    started = true;
    apply(backend.cached());
    void load(afterLoad);
  }

  async function openExternal(): Promise<void> {
    if (!backend.openExternal) return;
    try {
      await backend.openExternal();
      ioError.value = null;
    } catch (e) {
      ioError.value = describeIoError(e);
    }
  }

  return { text, ioError, apply, save, start, openExternal, canOpenExternal: backend.openExternal !== null };
}
