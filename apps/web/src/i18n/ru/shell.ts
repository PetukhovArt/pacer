import type { Translation } from '../types';
import type { shell as en } from '../en/shell';

export const shell: Translation<typeof en> = {
  nav: {
    label: 'Навигация',
    tasks: 'Задачи',
    agents: 'Рабочая область',
    reviews: 'Ревью',
    jira: 'Jira',
    confluence: 'Confluence',
    reviewsPending: 'Ждут моего ревью',
  },
  collapse: 'Свернуть панель',
  expand: 'Развернуть панель',
  resize: 'Изменить ширину панели',
  switchWorkspace: 'Перейти в {name}',
  topics: 'Топики',
  tasks: 'Задачи',
  online: 'в сети',
  offline: 'нет связи',
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
