import { createChannel, hasBackend, invoke, isDesktop } from '../bridge';

export type ConfigFile = 'settings' | 'keymap';

export type Backend = {
  cached: () => string;
  remember: (text: string) => void;
  read: () => Promise<string>;
  write: (text: string) => Promise<void>;
  watch: (onText: (text: string) => void) => void;
  openExternal: (() => Promise<void>) | null;
};

function appBackend(file: ConfigFile): Backend {
  const cacheKey = `veyra.${file}.cache`;
  return {
    cached: () => localStorage.getItem(cacheKey) ?? '',
    remember: (text) => localStorage.setItem(cacheKey, text),
    read: () => invoke<string>('settings_read', { file }),
    write: (text) => invoke<void>('settings_write', { text, file }),
    watch: (onText) => {
      void invoke<void>('settings_watch', { onEvent: createChannel<{ text: string }>((event) => onText(event.text)), file });
    },
    openExternal: isDesktop ? () => invoke<void>('settings_open_external', { file }) : null,
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
  return hasBackend ? appBackend(file) : browserBackend(file);
}
