import type { Translation } from '../types';
import type { commands as en } from '../en/commands';

export const commands: Translation<typeof en> = {
  category: {
    navigation: 'Переход',
    view: 'Вид',
    terminal: 'Терминал',
    keys: 'Клавиши',
  },
  action: {
    paletteOpen: 'Палитра команд',
    navAgents: 'Перейти в рабочую область',
    navReviews: 'Перейти к ревью',
    navJira: 'Перейти к Jira',
    navConfluence: 'Перейти к Confluence',
    navSettings: 'Открыть настройки',
    sidebarToggle: 'Свернуть или развернуть боковую панель',
    sidebarSearch: 'Найти задачу в боковой панели',
    themeToggle: 'Переключить тему',
    terminalZoomIn: 'Увеличить шрифт терминала',
    terminalZoomOut: 'Уменьшить шрифт терминала',
    terminalZoomReset: 'Сбросить размер шрифта терминала',
    agentsQuickOpen: 'Быстрый переход к сессии, запросу или задаче',
    keymapOpenSettings: 'Настроить сочетания клавиш',
    keymapOpenFile: 'Открыть keymap.json в редакторе',
  },
  palette: {
    label: 'Палитра команд',
    placeholder: 'Введите команду',
    search: 'Поиск команды',
    empty: 'Ничего не найдено',
  },
  problem: {
    badEntry: 'элемент {index}: ожидался объект { "context"?: строка, "bindings": { сочетание: действие | null } }',
    entryError: 'элемент {index}: {message}',
    badShortcut: '«{text}»: непонятное сочетание',
    unknownAction: '«{text}»: неизвестное действие {action}',
  },
  when: {
    unexpectedChar: 'неожиданный символ в «{source}» на позиции {position}',
    unterminated: 'условие «{source}» оборвано',
    missingParen: 'нет закрывающей скобки в «{source}»',
    expectedName: 'ожидалось имя, найдено «{token}» в «{source}»',
    expectedValue: 'после {operator} нужно значение в «{source}»',
    extra: 'лишнее «{token}» в «{source}»',
  },
};
