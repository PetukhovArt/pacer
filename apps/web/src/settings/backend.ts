import { Channel, invoke } from '@tauri-apps/api/core';

export type ConfigFile = 'settings' | 'keymap';

export type Backend = {
  cached: () => string;
  remember: (text: string) => void;
  read: () => Promise<string>;
  write: (text: string) => Promise<void>;
  watch: (onText: (text: string) => void) => void;
  openExternal: (() => Promise<void>) | null;
};

const inTauri = '__TAURI_INTERNALS__' in window;

function tauriBackend(file: ConfigFile): Backend {
  const cacheKey = `veyra.${file}.cache`;
  return {
    cached: () => localStorage.getItem(cacheKey) ?? '',
    remember: (text) => localStorage.setItem(cacheKey, text),
    read: () => invoke<string>('settings_read', { file }),
    write: (text) => invoke<void>('settings_write', { text, file }),
    watch: (onText) => {
      void invoke<void>('settings_watch', { onEvent: new Channel<{ text: string }>((event) => onText(event.text)), file });
    },
    openExternal: () => invoke<void>('settings_open_external', { file }),
  };
}

function browserBackend(file: ConfigFile): Backend {
  const fileKey = `veyra.${file}.json`;
  return {
    cached: () => localStorage.getItem(fileKey) ?? '',
    remember: () => {},
    read: () => Promise.resolve(localStorage.getItem(fileKey) ?? ''),
    write: (text) => {
      localStorage.setItem(fileKey, text);
      return Promise.resolve();
    },
    watch: (onText) => {
      window.addEventListener('storage', (event) => {
        if (event.key === fileKey) onText(event.newValue ?? '');
      });
    },
    openExternal: null,
  };
}

export function createBackend(file: ConfigFile): Backend {
  return inTauri ? tauriBackend(file) : browserBackend(file);
}
