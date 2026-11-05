import type { Translation } from '../types';
import type { files as en } from '../en/files';

export const files: Translation<typeof en> = {
  markdown: {
    viewMode: 'Режим просмотра',
    code: 'Код',
    split: 'Разделить',
    preview: 'Просмотр',
    toc: 'Содержание',
    openExternalTitle: 'Открыть в приложении по умолчанию',
    openExternal: 'Открыть в редакторе',
  },
  picker: {
    open: 'Открыть Markdown-файл',
    filter: 'Фильтр Markdown-файлов',
    none: 'В этом рабочем дереве нет Markdown-файлов',
  },
  switcher: {
    dialog: 'Быстрый переход к сессии или файлу',
    search: 'Поиск сессий и файлов',
    placeholderFiles: 'workspace › проект › ветка › сессия или Markdown-файл в {scope}',
  },
};
