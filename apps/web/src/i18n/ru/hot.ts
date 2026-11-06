export const hot = {
  title: 'Горячие сессии',
  bar: {
    label: 'Горячие сессии',
    pin: 'Закрепить',
    unpin: 'Открепить',
    empty: 'Нет живых сессий',
    more: 'Ещё сессий: {count}',
  },
  switcher: {
    dialog: 'Переключение сессии',
    empty: 'Нет живых сессий',
  },
  status: {
    waiting: 'Ждёт вас',
    working: 'Работает',
    idle: 'Простаивает',
    error: 'Ошибка',
  },
  overview: {
    title: 'Обзор горячих сессий',
    hint: 'Стрелки для перемещения, Enter чтобы открыть, Esc чтобы закрыть',
    open: 'Обзор горячих сессий (Ctrl+Shift+O)',
    close: 'Закрыть обзор',
    empty: 'Нет живых сессий',
    noPreview: 'Пока без активности',
  },
  toast: {
    waiting: 'Ждут: {count}',
    nobody: 'Никто не ждёт',
  },
  mode: {
    titlebar: 'В заголовке окна',
    off: 'Выключено',
  },
  setting: {
    label: 'Горячие сессии',
    description: 'Живые сессии под рукой: чипы в заголовке окна и обзор по Ctrl+Shift+O. Alt+1..9 открывает N-ю, Alt+J переходит к следующей, которая ждёт вас, Ctrl+Tab переключает между недавними.',
  },
} as const;
