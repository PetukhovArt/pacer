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
  editor: {
    dirty: 'Есть несохранённые изменения',
    saveTitle: 'Сохранить (Ctrl+S)',
    changed: {
      title: 'Файл изменён на диске',
      hint: 'Файл изменён вне Veyra, а у вас есть несохранённые правки.',
      reload: 'Перечитать',
      keep: 'Оставить мою (перезаписать)',
      compare: 'Сравнить',
    },
    compare: {
      disk: 'На диске',
      mine: 'Ваша версия',
    },
    unsaved: {
      title: 'Несохранённые изменения',
      message: 'Есть несохранённые изменения: {names}. Сохранить перед закрытием?',
      discard: 'Не сохранять',
    },
  },
  settings: {
    category: 'Файлы',
    autoSave: {
      label: 'Автосохранение Markdown-файлов',
      description: 'Выкл.: сохранение по Ctrl+S. При потере фокуса: сохранять, когда редактор теряет фокус. С задержкой: сохранять через секунду после последней правки.',
      off: 'Выкл.',
      onBlur: 'При потере фокуса',
      afterDelay: 'С задержкой',
    },
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
