import type { Translation } from '../types';
import type { shell as en } from '../en/shell';

export const shell: Translation<typeof en> = {
  nav: {
    label: 'Навигация',
    tasks: 'Задачи',
    agents: 'Агенты',
    reviews: 'Ревью',
    jira: 'Jira',
    confluence: 'Confluence',
    reviewsPending: 'Ждут моего ревью',
  },
  search: 'Найти задачу',
  collapse: 'Свернуть панель',
  expand: 'Развернуть панель',
  resize: 'Изменить ширину панели',
  switchWorkspace: 'Перейти в {name}',
  topics: 'Топики',
  newTopic: 'Новый топик',
  topicName: 'Название топика',
  topicPlaceholder: 'название топика',
  tasks: 'Задачи',
  unread: 'новое',
  nothingFound: 'Ничего не найдено',
  noOpenTasks: 'На вас нет открытых задач',
  online: 'в сети',
  offline: 'нет связи',
  settings: 'Настройки',
  themeLight: 'Светлая тема',
  themeDark: 'Тёмная тема',
};
