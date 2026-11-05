import { shallowRef } from 'vue';
import { backend } from './backend';
import { describeFailure, parseSettings, type ParseFailure } from './parse';
import { SETTINGS, checkValue, coerceValue, defaultValues, isSettingKey, type SettingKey, type SettingValues } from './schema';

const LEGACY_THEME_KEY = 'veyra.theme';
const LEGACY_TERMINAL_SIZE_KEY = 'veyra.terminal.fontSize';
const ECHO_MEMORY = 8;

const text = shallowRef('');
const fileObject = shallowRef<Record<string, unknown>>({});
const values = shallowRef<SettingValues>(defaultValues());
const parseError = shallowRef<ParseFailure | null>(null);
const issues = shallowRef<string[]>([]);
const ioError = shallowRef<string | null>(null);

let started = false;
let pending: string | null = null;
let flushing = false;
const written: string[] = [];

const describe = (e: unknown) => (e instanceof Error ? e.message : String(e));

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

function applyText(next: string): void {
  text.value = next;
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

async function flush(): Promise<void> {
  if (flushing) return;
  flushing = true;
  try {
    while (pending !== null) {
      const next = pending;
      pending = null;
      await backend.write(next);
    }
    ioError.value = null;
  } catch (e) {
    ioError.value = describe(e);
  } finally {
    flushing = false;
  }
}

function persist(next: string): void {
  written.push(next);
  if (written.length > ECHO_MEMORY) written.shift();
  pending = next;
  void flush();
}

function commit(object: Record<string, unknown>): void {
  const next = `${JSON.stringify(object, null, 2)}\n`;
  applyText(next);
  persist(next);
}

function onDisk(next: string): void {
  if (next === text.value) {
    written.length = 0;
    return;
  }
  if (written.includes(next)) return;
  written.length = 0;
  applyText(next);
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

async function load(): Promise<void> {
  try {
    const disk = await backend.read();
    if (disk !== text.value) applyText(disk);
  } catch (e) {
    ioError.value = describe(e);
    return;
  }
  migrateLegacy();
  backend.watch(onDisk);
}

function start(): void {
  if (started) return;
  started = true;
  applyText(backend.cached());
  void load();
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
  applyText(candidate);
  persist(candidate);
  return null;
}

async function openExternal(): Promise<void> {
  if (!backend.openExternal) return;
  try {
    await backend.openExternal();
    ioError.value = null;
  } catch (e) {
    ioError.value = describe(e);
  }
}

const api = { values, text, parseError, issues, ioError, set, reset, isModified, checkText, saveText, openExternal, canOpenExternal: backend.openExternal !== null };

export function useSettings(): typeof api {
  start();
  return api;
}
