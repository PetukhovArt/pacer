import type { Translation } from '../types';
import type { mobile as en } from '../en/mobile';

export const mobile: Translation<typeof en> = {
  back: 'Назад',
  navLabel: 'Разделы',
  keysLabel: 'Клавиши терминала',
  key: {
    esc: 'Esc',
    tab: 'Tab',
    ctrl: 'Ctrl',
    up: 'Вверх',
    down: 'Вниз',
    left: 'Влево',
    right: 'Вправо',
    newline: 'Shift+Enter',
  },
};
