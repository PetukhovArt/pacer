type Category = 'appearance' | 'terminal' | 'sessions' | 'connectors';
type Option = { readonly value: string; readonly label: string };
type Base = { readonly label: string; readonly description: string; readonly category: Category };
type BooleanDef = Base & { readonly type: 'boolean'; readonly default: boolean };
type NumberDef = Base & { readonly type: 'number'; readonly default: number; readonly min: number; readonly max: number; readonly step: number };
type StringDef = Base & { readonly type: 'string'; readonly default: string; readonly placeholder?: string };
type EnumDef = Base & { readonly type: 'enum'; readonly default: string; readonly options: readonly Option[] };
export type SettingDef = BooleanDef | NumberDef | StringDef | EnumDef;

export const CATEGORIES: readonly { readonly id: Category; readonly label: string }[] = [
  { id: 'appearance', label: 'Внешний вид' },
  { id: 'terminal', label: 'Терминал' },
  { id: 'sessions', label: 'Сессии' },
  { id: 'connectors', label: 'Коннекторы' },
];

const FALLBACK_MONO = "'JetBrains Mono', Consolas, monospace";
const monoFont = getComputedStyle(document.documentElement).getPropertyValue('--vr-font-mono').trim() || FALLBACK_MONO;

export const SETTINGS = {
  theme: {
    type: 'enum',
    default: 'dark',
    label: 'Тема',
    description: 'Цветовая схема приложения. «Как в системе» следует настройке ОС.',
    category: 'appearance',
    options: [
      { value: 'dark', label: 'Тёмная' },
      { value: 'light', label: 'Светлая' },
      { value: 'system', label: 'Как в системе' },
    ],
  },
  'ui.font_size': {
    type: 'number',
    default: 14,
    label: 'Размер шрифта интерфейса',
    description: 'Базовый размер текста интерфейса в пикселях. Терминал настраивается отдельно.',
    category: 'appearance',
    min: 12,
    max: 20,
    step: 1,
  },
  'terminal.font_family': {
    type: 'string',
    default: monoFont,
    label: 'Шрифт терминала',
    description: 'Список семейств шрифтов через запятую, как в CSS font-family.',
    category: 'terminal',
  },
  'terminal.font_size': {
    type: 'number',
    default: 13,
    label: 'Размер шрифта терминала',
    description: 'Размер текста в терминале в пикселях. Меняется также Ctrl+колесо и Ctrl+=/−/0.',
    category: 'terminal',
    min: 8,
    max: 32,
    step: 1,
  },
  'terminal.line_height': {
    type: 'number',
    default: 1,
    label: 'Межстрочный интервал терминала',
    description: 'Множитель высоты строки относительно размера шрифта.',
    category: 'terminal',
    min: 1,
    max: 2,
    step: 0.05,
  },
  'terminal.scrollback': {
    type: 'number',
    default: 10000,
    label: 'История терминала',
    description: 'Сколько строк прокрутки хранит каждый терминал.',
    category: 'terminal',
    min: 0,
    max: 100000,
    step: 1000,
  },
  'terminal.cursor_blink': {
    type: 'boolean',
    default: true,
    label: 'Мигающий курсор',
    description: 'Мигание курсора в терминале.',
    category: 'terminal',
  },
  'sessions.default_kind': {
    type: 'enum',
    default: 'claude',
    label: 'Тип новой сессии',
    description: 'Какой тип выбран по умолчанию в форме новой сессии.',
    category: 'sessions',
    options: [
      { value: 'claude', label: 'Claude' },
      { value: 'codex', label: 'Codex' },
      { value: 'cursor', label: 'Cursor' },
      { value: 'terminal', label: 'Терминал' },
    ],
  },
  'sessions.default_model': {
    type: 'string',
    default: '',
    label: 'Модель по умолчанию',
    description: 'Подставляется в форму новой сессии. Пусто — модель выбирает сам агент.',
    category: 'sessions',
    placeholder: 'по умолчанию',
  },
  'sessions.default_effort': {
    type: 'string',
    default: '',
    label: 'Effort по умолчанию',
    description: 'Подставляется в форму новой сессии. Пусто — значение выбирает сам агент.',
    category: 'sessions',
    placeholder: 'по умолчанию',
  },
} as const satisfies Record<string, SettingDef>;

export type SettingKey = keyof typeof SETTINGS;

type ValueOf<D> = D extends { type: 'boolean' } ? boolean : D extends { type: 'number' } ? number : D extends { type: 'enum'; options: readonly { value: infer V }[] } ? V : string;
export type SettingValues = { [K in SettingKey]: ValueOf<(typeof SETTINGS)[K]> };

export const SETTING_KEYS = Object.keys(SETTINGS) as SettingKey[];

export function isSettingKey(key: string): key is SettingKey {
  return Object.hasOwn(SETTINGS, key);
}

export function defaultValues(): SettingValues {
  return Object.fromEntries(SETTING_KEYS.map((key) => [key, SETTINGS[key].default])) as SettingValues;
}

export function checkValue(def: SettingDef, raw: unknown): string | null {
  switch (def.type) {
    case 'boolean':
      return typeof raw === 'boolean' ? null : 'ожидается true или false';
    case 'number':
      if (typeof raw !== 'number' || !Number.isFinite(raw)) return 'ожидается число';
      return raw < def.min || raw > def.max ? `число от ${def.min} до ${def.max}` : null;
    case 'string':
      return typeof raw === 'string' ? null : 'ожидается строка';
    case 'enum':
      return typeof raw === 'string' && def.options.some((option) => option.value === raw) ? null : `одно из: ${def.options.map((option) => option.value).join(', ')}`;
  }
}

export function coerceValue(def: SettingDef, raw: unknown): SettingValues[SettingKey] | undefined {
  if (def.type === 'number' && typeof raw === 'number' && Number.isFinite(raw)) {
    const clamped = Math.min(def.max, Math.max(def.min, raw));
    return Number.isInteger(def.step) ? Math.round(clamped) : Math.round(clamped * 1000) / 1000;
  }
  return checkValue(def, raw) === null ? (raw as SettingValues[SettingKey]) : undefined;
}
