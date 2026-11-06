import type { Translation } from '../types';
import type { confluence as en } from '../en/confluence';

export const confluence: Translation<typeof en> = {
  view: {
    empty: 'Подключите Confluence, чтобы искать и читать страницы здесь',
    placeholder: 'Найдите страницу слева',
  },
  search: {
    placeholder: 'Поиск по Confluence',
    noHits: 'По запросу «{query}» ничего не найдено',
    recent: 'Недавние страницы',
    prompt: 'Введите запрос, чтобы найти страницу',
  },
  page: {
    path: 'Путь',
    version: 'версия {version}',
    contents: 'Содержание',
  },
};
