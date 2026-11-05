import type { Translation } from '../types';
import type { common as en } from '../en/common';

export const common: Translation<typeof en> = {
  cancel: 'Отмена',
  save: 'Сохранить',
  close: 'Закрыть',
  delete: 'Удалить',
  remove: 'Убрать',
  create: 'Создать',
  confirm: 'Подтвердить',
  rename: 'Переименовать',
  retry: 'Повторить',
  refresh: 'Обновить',
  copy: 'Копировать',
  back: 'Назад',
  open: 'Открыть',
  openInBrowser: 'Открыть в браузере',
  search: 'Поиск',
  loading: 'Загрузка…',
  error: 'Ошибка',
  noResults: 'Ничего не найдено',
  time: {
    justNow: 'только что',
  },
  language: {
    label: 'Язык',
    en: 'English',
    ru: 'Русский',
  },
};
