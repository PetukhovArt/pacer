import type { Translation } from '../types';
import type { shell as en } from '../en/shell';

export const shell: Translation<typeof en> = {
  nav: {
    label: 'Навигация',
    agents: 'Рабочая область',
    reviews: 'Ревью',
    jira: 'Jira',
    confluence: 'Confluence',
    reviewsPending: 'Ждут моего ревью',
  },
  collapse: 'Свернуть панель',
  expand: 'Развернуть панель',
  resize: 'Изменить ширину панели',
  settings: 'Настройки',
  themeLight: 'Светлая тема',
  themeDark: 'Тёмная тема',
  window: {
    minimize: 'Свернуть',
    maximize: 'Развернуть',
    restore: 'Восстановить',
    close: 'Закрыть',
  },
};
