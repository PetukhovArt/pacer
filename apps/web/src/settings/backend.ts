import { Channel, invoke } from '@tauri-apps/api/core';

type Backend = {
  cached: () => string;
  remember: (text: string) => void;
  read: () => Promise<string>;
  write: (text: string) => Promise<void>;
  watch: (onText: (text: string) => void) => void;
  openExternal: (() => Promise<void>) | null;
};

const FILE_KEY = 'veyra.settings.json';
const CACHE_KEY = 'veyra.settings.cache';

const tauriBackend: Backend = {
  cached: () => localStorage.getItem(CACHE_KEY) ?? '',
  remember: (text) => localStorage.setItem(CACHE_KEY, text),
  read: () => invoke<string>('settings_read'),
  write: (text) => invoke<void>('settings_write', { text }),
  watch: (onText) => {
    void invoke<void>('settings_watch', { onEvent: new Channel<{ text: string }>((event) => onText(event.text)) });
  },
  openExternal: () => invoke<void>('settings_open_external'),
};

const browserBackend: Backend = {
  cached: () => localStorage.getItem(FILE_KEY) ?? '',
  remember: () => {},
  read: () => Promise.resolve(localStorage.getItem(FILE_KEY) ?? ''),
  write: (text) => {
    localStorage.setItem(FILE_KEY, text);
    return Promise.resolve();
  },
  watch: (onText) => {
    window.addEventListener('storage', (event) => {
      if (event.key === FILE_KEY) onText(event.newValue ?? '');
    });
  },
  openExternal: null,
};

export const backend: Backend = '__TAURI_INTERNALS__' in window ? tauriBackend : browserBackend;
