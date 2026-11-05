import type { Translation } from '../types';
import type { settings as en } from '../en/settings';

export const settings: Translation<typeof en> = {
  title: 'Настройки',
  search: 'Найти настройку',
  openJson: 'Открыть settings.json',
  backToSettings: 'К настройкам',
  openExternal: 'Открыть в редакторе',
  openExternalTitle: 'Открыть файл в редакторе по умолчанию',
  appOnly: 'Доступно только в приложении',
  categoriesLabel: 'Категории настроек',
  empty: 'Ничего не найдено',
  parseError: 'settings.json не разобран, действуют последние верные значения.',
  issues: 'Для этих ключей взято значение по умолчанию:',
  resetTitle: 'Сбросить к значению по умолчанию',
  resetLabel: 'Сбросить',
  connectorKeywords: 'коннекторы коннектор github gitlab jira confluence токен токены подключение аккаунт',
  category: {
    appearance: 'Внешний вид',
    terminal: 'Терминал',
    sessions: 'Сессии',
    connectors: 'Коннекторы',
    keymap: 'Клавиши',
  },
  def: {
    theme: {
      label: 'Тема',
      description: 'Цветовая схема приложения. «Как в системе» следует настройке ОС.',
      dark: 'Тёмная',
      light: 'Светлая',
      system: 'Как в системе',
    },
    language: {
      label: 'Язык интерфейса',
      description: 'Язык меню, кнопок и сообщений',
    },
    fontSize: {
      label: 'Размер шрифта интерфейса',
      description: 'Базовый размер текста интерфейса в пикселях. Терминал настраивается отдельно.',
    },
    animations: {
      label: 'Анимации',
      description: 'Плавные переходы между экранами и анимация списков. Отключите, чтобы убрать движение.',
    },
    terminalFontFamily: {
      label: 'Шрифт терминала',
      description: 'Список семейств шрифтов через запятую, как в CSS font-family.',
    },
    terminalFontSize: {
      label: 'Размер шрифта терминала',
      description: 'Размер текста в терминале в пикселях. Меняется также Ctrl+колесо и Ctrl+=/−/0.',
    },
    terminalLineHeight: {
      label: 'Межстрочный интервал терминала',
      description: 'Множитель высоты строки относительно размера шрифта.',
    },
    terminalScrollback: {
      label: 'История терминала',
      description: 'Сколько строк прокрутки хранит каждый терминал.',
    },
    terminalCursorBlink: {
      label: 'Мигающий курсор',
      description: 'Мигание курсора в терминале.',
    },
    sessionsDefaultKind: {
      label: 'Тип новой сессии',
      description: 'Какой тип выбран по умолчанию в форме новой сессии.',
      claude: 'Claude',
      codex: 'Codex',
      cursor: 'Cursor',
      terminal: 'Терминал',
    },
    sessionsDefaultModel: {
      label: 'Модель по умолчанию',
      description: 'Подставляется в форму новой сессии. Пусто — модель выбирает сам агент.',
      placeholder: 'по умолчанию',
    },
    sessionsDefaultEffort: {
      label: 'Effort по умолчанию',
      description: 'Подставляется в форму новой сессии. Пусто — значение выбирает сам агент.',
      placeholder: 'по умолчанию',
    },
  },
  problem: {
    expectedBoolean: 'ожидается true или false',
    expectedNumber: 'ожидается число',
    range: 'число от {min} до {max}',
    expectedString: 'ожидается строка',
    oneOf: 'одно из: {values}',
  },
  parse: {
    rootObject: 'Корень файла должен быть объектом {…}',
    rootArray: 'Корень файла должен быть массивом […]',
    position: 'строка {line}, столбец {column}: {message}',
  },
  json: {
    unsaved: 'есть несохранённые изменения',
    saved: 'сохранено',
  },
  keymap: {
    search: 'Найти действие или сочетание',
    openJson: 'Открыть keymap.json',
    backToList: 'К списку',
    parseError: 'keymap.json не разобран, действуют последние верные значения.',
    skipped: 'Эти записи пропущены:',
    recording: 'Нажмите сочетание, Esc отменяет',
    unassigned: 'не назначено',
    edit: 'Изменить',
    reset: 'Сбросить',
    conflict: '{shortcut} уже назначено: {actions}. Если назначить, у них это сочетание перестанет работать.',
    force: 'Всё равно назначить',
    empty: 'Ничего не найдено',
    contextTerminal: 'в терминале',
    contextOutsideTerminal: 'вне терминала',
  },
};
