import { shallowRef } from 'vue';
import { createBackend } from './backend';
import { createFileText } from './fileText';
import { describeFailure, parseSettings, type ParseFailure } from './parse';
import { SETTINGS, checkValue, coerceValue, defaultValues, isSettingKey, type SettingKey, type SettingValues } from './schema';

const LEGACY_THEME_KEY = 'veyra.theme';
const LEGACY_TERMINAL_SIZE_KEY = 'veyra.terminal.fontSize';

const fileObject = shallowRef<Record<string, unknown>>({});
const values = shallowRef<SettingValues>(defaultValues());
const parseError = shallowRef<ParseFailure | null>(null);
const issues = shallowRef<string[]>([]);

const backend = createBackend('settings');
const file = createFileText(backend, parseText);
const { text, ioError } = file;

function resolve(object: Record<string, unknown>): { values: SettingValues; issues: string[] } {
  const next = defaultValues() as Record<SettingKey, unknown>;
  const found: string[] = [];
  for (const [key, raw] of Object.entries(object)) {
    if (!isSettingKey(key)) continue;
    const problem = checkValue(SETTINGS[key], raw);
    if (problem) found.push(`${key}: ${problem}`);
    else next[key] = raw;
  }
  return { values: next as SettingValues, issues: found };
}

function parseText(next: string): void {
  const result = parseSettings(next);
  if (!result.ok) {
    parseError.value = result.error;
    return;
  }
  parseError.value = null;
  fileObject.value = result.object;
  const resolved = resolve(result.object);
  values.value = resolved.values;
  issues.value = resolved.issues;
  backend.remember(next);
}

function commit(object: Record<string, unknown>): void {
  file.save(`${JSON.stringify(object, null, 2)}\n`);
}

function migrateLegacy(): void {
  if (parseError.value) return;
  const patch: Record<string, unknown> = {};
  const absent = (key: SettingKey) => !(key in fileObject.value);
  const theme = localStorage.getItem(LEGACY_THEME_KEY);
  if (theme !== null) {
    if (theme === 'light' && absent('theme')) patch.theme = 'light';
    localStorage.removeItem(LEGACY_THEME_KEY);
  }
  const size = localStorage.getItem(LEGACY_TERMINAL_SIZE_KEY);
  if (size !== null) {
    const value = coerceValue(SETTINGS['terminal.font_size'], Number(size));
    if (value !== undefined && value !== SETTINGS['terminal.font_size'].default && absent('terminal.font_size')) patch['terminal.font_size'] = value;
    localStorage.removeItem(LEGACY_TERMINAL_SIZE_KEY);
  }
  if (Object.keys(patch).length) commit({ ...fileObject.value, ...patch });
}

function start(): void {
  file.start(migrateLegacy);
}

function set(key: SettingKey, value: unknown): void {
  if (parseError.value) return;
  const def = SETTINGS[key];
  const next = coerceValue(def, value);
  if (next === undefined) return;
  const object = { ...fileObject.value };
  if (next === def.default) delete object[key];
  else object[key] = next;
  if (!Object.keys(object).length && !text.value.trim()) return;
  commit(object);
}

function reset(key: SettingKey): void {
  set(key, SETTINGS[key].default);
}

function isModified(key: SettingKey): boolean {
  return values.value[key] !== SETTINGS[key].default;
}

function checkText(candidate: string): string | null {
  const result = parseSettings(candidate);
  if (!result.ok) return describeFailure(result.error);
  const found = resolve(result.object).issues;
  return found.length ? found.join('; ') : null;
}

function saveText(candidate: string): string | null {
  const problem = checkText(candidate);
  if (problem) return problem;
  file.save(candidate);
  return null;
}

const api = { values, text, parseError, issues, ioError, set, reset, isModified, checkText, saveText, openExternal: file.openExternal, canOpenExternal: file.canOpenExternal };

export function useSettings(): typeof api {
  start();
  return api;
}
