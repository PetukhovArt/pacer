import type { MessageKey } from '../i18n';
import { tr } from './translate';

type Category = 'appearance' | 'terminal' | 'sessions' | 'connectors' | 'remote' | 'keymap';
type Option = { readonly value: string; readonly label: MessageKey };
type Base = { readonly label: MessageKey; readonly description: MessageKey; readonly category: Category };
type BooleanDef = Base & { readonly type: 'boolean'; readonly default: boolean };
type NumberDef = Base & { readonly type: 'number'; readonly default: number; readonly min: number; readonly max: number; readonly step: number };
type StringDef = Base & { readonly type: 'string'; readonly default: string; readonly placeholder?: MessageKey };
type EnumDef = Base & { readonly type: 'enum'; readonly default: string; readonly options: readonly Option[] };
type MapDef = Base & { readonly type: 'map'; readonly default: Readonly<Record<string, string>>; readonly placeholder?: MessageKey };
export type SettingDef = BooleanDef | NumberDef | StringDef | EnumDef | MapDef;

export const CATEGORIES = [
  { id: 'appearance', label: 'settings.category.appearance' },
  { id: 'terminal', label: 'settings.category.terminal' },
  { id: 'sessions', label: 'settings.category.sessions' },
  { id: 'connectors', label: 'settings.category.connectors' },
  { id: 'remote', label: 'remote.category' },
  { id: 'keymap', label: 'settings.category.keymap' },
] as const satisfies readonly { readonly id: Category; readonly label: MessageKey }[];

const FALLBACK_MONO = "'JetBrains Mono', Consolas, monospace";
const monoFont = getComputedStyle(document.documentElement).getPropertyValue('--vr-font-mono').trim() || FALLBACK_MONO;

export const SETTINGS = {
  theme: {
    type: 'enum',
    default: 'dark',
    label: 'settings.def.theme.label',
    description: 'settings.def.theme.description',
    category: 'appearance',
    options: [
      { value: 'dark', label: 'settings.def.theme.dark' },
      { value: 'light', label: 'settings.def.theme.light' },
      { value: 'system', label: 'settings.def.theme.system' },
    ],
  },
  'ui.language': {
    type: 'enum',
    default: 'en',
    label: 'settings.def.language.label',
    description: 'settings.def.language.description',
    category: 'appearance',
    options: [
      { value: 'en', label: 'common.language.en' },
      { value: 'ru', label: 'common.language.ru' },
    ],
  },
  'ui.font_size': {
    type: 'number',
    default: 14,
    label: 'settings.def.fontSize.label',
    description: 'settings.def.fontSize.description',
    category: 'appearance',
    min: 12,
    max: 20,
    step: 1,
  },
  'ui.animations': {
    type: 'boolean',
    default: true,
    label: 'settings.def.animations.label',
    description: 'settings.def.animations.description',
    category: 'appearance',
  },
  'terminal.font_family': {
    type: 'string',
    default: monoFont,
    label: 'settings.def.terminalFontFamily.label',
    description: 'settings.def.terminalFontFamily.description',
    category: 'terminal',
  },
  'terminal.font_size': {
    type: 'number',
    default: 13,
    label: 'settings.def.terminalFontSize.label',
    description: 'settings.def.terminalFontSize.description',
    category: 'terminal',
    min: 8,
    max: 32,
    step: 1,
  },
  'terminal.line_height': {
    type: 'number',
    default: 1,
    label: 'settings.def.terminalLineHeight.label',
    description: 'settings.def.terminalLineHeight.description',
    category: 'terminal',
    min: 1,
    max: 2,
    step: 0.05,
  },
  'terminal.scrollback': {
    type: 'number',
    default: 10000,
    label: 'settings.def.terminalScrollback.label',
    description: 'settings.def.terminalScrollback.description',
    category: 'terminal',
    min: 0,
    max: 100000,
    step: 1000,
  },
  'terminal.cursor_blink': {
    type: 'boolean',
    default: true,
    label: 'settings.def.terminalCursorBlink.label',
    description: 'settings.def.terminalCursorBlink.description',
    category: 'terminal',
  },
  'sessions.default_kind': {
    type: 'enum',
    default: 'claude',
    label: 'settings.def.sessionsDefaultKind.label',
    description: 'settings.def.sessionsDefaultKind.description',
    category: 'sessions',
    options: [
      { value: 'claude', label: 'settings.def.sessionsDefaultKind.claude' },
      { value: 'codex', label: 'settings.def.sessionsDefaultKind.codex' },
      { value: 'cursor', label: 'settings.def.sessionsDefaultKind.cursor' },
      { value: 'terminal', label: 'settings.def.sessionsDefaultKind.terminal' },
    ],
  },
  'sessions.default_model': {
    type: 'string',
    default: '',
    label: 'settings.def.sessionsDefaultModel.label',
    description: 'settings.def.sessionsDefaultModel.description',
    category: 'sessions',
    placeholder: 'settings.def.sessionsDefaultModel.placeholder',
  },
  'sessions.default_effort': {
    type: 'string',
    default: '',
    label: 'settings.def.sessionsDefaultEffort.label',
    description: 'settings.def.sessionsDefaultEffort.description',
    category: 'sessions',
    placeholder: 'settings.def.sessionsDefaultEffort.placeholder',
  },
  'tree.jiraProjects': {
    type: 'map',
    default: {},
    label: 'settings.def.treeJiraProjects.label',
    description: 'settings.def.treeJiraProjects.description',
    category: 'connectors',
    placeholder: 'settings.def.treeJiraProjects.placeholder',
  },
  'remote.enabled': {
    type: 'boolean',
    default: false,
    label: 'remote.def.enabled.label',
    description: 'remote.def.enabled.description',
    category: 'remote',
  },
  'remote.bind': {
    type: 'enum',
    default: 'localhost',
    label: 'remote.def.bind.label',
    description: 'remote.def.bind.description',
    category: 'remote',
    options: [
      { value: 'localhost', label: 'remote.def.bind.localhost' },
      { value: 'lan', label: 'remote.def.bind.lan' },
      { value: 'tailscale', label: 'remote.def.bind.tailscale' },
    ],
  },
  'remote.port': {
    type: 'number',
    default: 4477,
    label: 'remote.def.port.label',
    description: 'remote.def.port.description',
    category: 'remote',
    min: 1024,
    max: 65535,
    step: 1,
  },
} as const satisfies Record<string, SettingDef>;

export type SettingKey = keyof typeof SETTINGS;

type ValueOf<D> = D extends { type: 'boolean' }
  ? boolean
  : D extends { type: 'number' }
    ? number
    : D extends { type: 'enum'; options: readonly { value: infer V }[] }
      ? V
      : D extends { type: 'map' }
        ? Readonly<Record<string, string>>
        : string;
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
      return typeof raw === 'boolean' ? null : tr('settings.problem.expectedBoolean');
    case 'number':
      if (typeof raw !== 'number' || !Number.isFinite(raw)) return tr('settings.problem.expectedNumber');
      return raw < def.min || raw > def.max ? tr('settings.problem.range', { min: def.min, max: def.max }) : null;
    case 'string':
      return typeof raw === 'string' ? null : tr('settings.problem.expectedString');
    case 'enum':
      return typeof raw === 'string' && def.options.some((option) => option.value === raw) ? null : tr('settings.problem.oneOf', { values: def.options.map((option) => option.value).join(', ') });
    case 'map':
      return typeof raw === 'object' && raw !== null && !Array.isArray(raw) && Object.values(raw).every((value) => typeof value === 'string') ? null : tr('settings.problem.expectedMap');
  }
}

export function coerceValue(def: SettingDef, raw: unknown): SettingValues[SettingKey] | undefined {
  if (def.type === 'number' && typeof raw === 'number' && Number.isFinite(raw)) {
    const clamped = Math.min(def.max, Math.max(def.min, raw));
    return Number.isInteger(def.step) ? Math.round(clamped) : Math.round(clamped * 1000) / 1000;
  }
  return checkValue(def, raw) === null ? (raw as SettingValues[SettingKey]) : undefined;
}
