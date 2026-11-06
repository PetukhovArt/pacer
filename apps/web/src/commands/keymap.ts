import { computed, shallowRef } from 'vue';
import { createBackend } from '../settings/backend';
import { createFileText } from '../settings/fileText';
import { describeFailure, parseJsonArray, type ParseFailure } from '../settings/parse';
import { tr } from '../settings/translate';
import { DEFAULT_KEYMAP, type KeymapEntry } from './defaults';
import { parseSequence, sequenceKey, sequenceLabel, type Chord } from './keys';
import { getAction } from './registry';
import { parseCondition, type Condition, type Context } from './when';

type Origin = 'default' | 'user';

type Binding = {
  origin: Origin;
  context: string | null;
  condition: Condition | null;
  sequence: Chord[];
  key: string;
  action: string | null;
};

export type Shortcut = { key: string; label: string; context: string | null; origin: Origin };

type Compiled = { bindings: Binding[]; problems: string[] };

const backend = createBackend('keymap');
const raw = shallowRef<unknown[]>([]);
const parseError = shallowRef<ParseFailure | null>(null);

function parseText(next: string): void {
  const result = parseJsonArray(next);
  if (!result.ok) {
    parseError.value = result.error;
    return;
  }
  parseError.value = null;
  raw.value = result.list;
  backend.remember(next);
}

const file = createFileText(backend, parseText);

function asEntry(value: unknown): KeymapEntry | null {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return null;
  const { context, bindings } = value as { context?: unknown; bindings?: unknown };
  if (context !== undefined && typeof context !== 'string') return null;
  if (typeof bindings !== 'object' || bindings === null || Array.isArray(bindings)) return null;
  if (!Object.values(bindings).every((action) => action === null || typeof action === 'string')) return null;
  return context === undefined ? { bindings: bindings as KeymapEntry['bindings'] } : { context, bindings: bindings as KeymapEntry['bindings'] };
}

function compile(list: readonly unknown[], origin: Origin): Compiled {
  const result: Compiled = { bindings: [], problems: [] };
  list.forEach((item, index) => {
    const entry = asEntry(item);
    if (!entry) {
      result.problems.push(tr('commands.problem.badEntry', { index: index + 1 }));
      return;
    }
    let condition: Condition | null = null;
    if (entry.context !== undefined) {
      try {
        condition = parseCondition(entry.context);
      } catch (e) {
        result.problems.push(tr('commands.problem.entryError', { index: index + 1, message: e instanceof Error ? e.message : String(e) }));
        return;
      }
    }
    for (const [text, action] of Object.entries(entry.bindings)) {
      const sequence = parseSequence(text);
      if (!sequence) {
        result.problems.push(tr('commands.problem.badShortcut', { text }));
        continue;
      }
      if (action !== null && !getAction(action)) result.problems.push(tr('commands.problem.unknownAction', { text, action }));
      result.bindings.push({ origin, context: entry.context ?? null, condition, sequence, key: sequenceKey(sequence), action });
    }
  });
  return result;
}

const defaults = compile(DEFAULT_KEYMAP, 'default').bindings;
const user = computed(() => compile(raw.value, 'user'));
const all = computed(() => [...defaults, ...user.value.bindings]);

const live = computed(() => {
  const result: Binding[] = [];
  for (const binding of all.value) {
    for (let i = result.length - 1; i >= 0; i--) {
      const previous = result[i] as Binding;
      if (previous.key === binding.key && (binding.context === null || previous.context === binding.context)) result.splice(i, 1);
    }
    if (binding.action !== null) result.push(binding);
  }
  return result;
});

const byAction = computed(() => {
  const map = new Map<string, Shortcut[]>();
  for (const binding of live.value) {
    if (binding.action === null) continue;
    const list = map.get(binding.action) ?? [];
    list.push({ key: binding.key, label: sequenceLabel(binding.sequence), context: binding.context, origin: binding.origin });
    map.set(binding.action, list);
  }
  return map;
});

export const keymapIssues = computed(() => user.value.problems);
export const keymapParseError = parseError;
export const keymapText = file.text;
export const keymapIoError = file.ioError;
export const canOpenKeymapExternal = file.canOpenExternal;
export const openKeymapExternal = file.openExternal;

export function startKeymap(): void {
  file.start();
}

export function shortcutsOf(actionId: string): readonly Shortcut[] {
  return byAction.value.get(actionId) ?? [];
}

export function shortcutLabel(actionId: string): string {
  return shortcutsOf(actionId)[0]?.label ?? '';
}

export function resolveSequence(context: Context, sequence: readonly Chord[]): { exact: string | null; longer: boolean } {
  const table = new Map<string, string | null>();
  for (const binding of all.value) {
    if (!binding.condition || binding.condition(context)) table.set(binding.key, binding.action);
  }
  const key = sequenceKey(sequence);
  const prefix = `${key} `;
  let longer = false;
  for (const [candidate, action] of table) {
    if (action !== null && candidate.startsWith(prefix)) longer = true;
  }
  return { exact: table.get(key) ?? null, longer };
}

const defaultKeysOf = (actionId: string) => defaults.filter((binding) => binding.action === actionId).map((binding) => binding.key);

const normalize = (text: string) => {
  const sequence = parseSequence(text);
  return sequence ? sequenceKey(sequence) : text;
};

export function isModified(actionId: string): boolean {
  const keys = defaultKeysOf(actionId);
  return user.value.bindings.some((binding) => binding.action === actionId || (binding.action === null && keys.includes(binding.key)));
}

export function conflictsFor(actionId: string, sequence: readonly Chord[]): string[] {
  const key = sequenceKey(sequence);
  const found = new Set<string>();
  for (const binding of live.value) {
    if (binding.action === null || binding.action === actionId) continue;
    if (binding.key === key || binding.key.startsWith(`${key} `) || key.startsWith(`${binding.key} `)) found.add(binding.action);
  }
  return [...found];
}

function mapEntries(list: readonly unknown[], edit: (entry: KeymapEntry) => KeymapEntry): unknown[] {
  return list
    .map((item) => {
      const entry = asEntry(item);
      return entry ? edit(entry) : item;
    })
    .filter((item) => {
      const entry = asEntry(item);
      return !entry || Object.keys(entry.bindings).length > 0;
    });
}

function withBindings(entry: KeymapEntry, keep: (key: string, action: string | null, normalized: string) => boolean): KeymapEntry {
  const bindings = Object.fromEntries(Object.entries(entry.bindings).filter(([text, action]) => keep(text, action, normalize(text))));
  return entry.context === undefined ? { bindings } : { context: entry.context, bindings };
}

function commit(list: unknown[]): void {
  if (!list.length && !file.text.value.trim()) return;
  file.save(`${JSON.stringify(list, null, 2)}\n`);
}

export function resetAction(actionId: string): void {
  if (parseError.value) return;
  const keys = defaultKeysOf(actionId);
  commit(mapEntries(raw.value, (entry) => withBindings(entry, (_text, action, normalized) => action !== actionId && !(action === null && keys.includes(normalized)))));
}

export function rebind(actionId: string, sequence: readonly Chord[]): void {
  if (parseError.value) return;
  const key = sequenceKey(sequence);
  const keys = defaultKeysOf(actionId);
  if (keys.length === 1 && keys[0] === key) {
    resetAction(actionId);
    return;
  }
  const list = mapEntries(raw.value, (entry) => withBindings(entry, (_text, action, normalized) => action !== actionId && !(entry.context === undefined && normalized === key)));
  const taken = new Set(list.flatMap((item) => Object.keys(asEntry(item)?.bindings ?? {}).map(normalize)));
  const additions: Record<string, string | null> = {};
  for (const stale of keys) {
    if (stale !== key && !taken.has(stale)) additions[stale] = null;
  }
  additions[key] = actionId;
  const last = list.at(-1);
  const tail = last === undefined ? null : asEntry(last);
  if (tail && tail.context === undefined) list[list.length - 1] = { bindings: { ...tail.bindings, ...additions } };
  else list.push({ bindings: additions });
  commit(list);
}

function checkKeymapText(candidate: string): string | null {
  const result = parseJsonArray(candidate);
  if (!result.ok) return describeFailure(result.error);
  const { problems } = compile(result.list, 'user');
  return problems.length ? problems.join('; ') : null;
}

export function saveKeymapText(candidate: string): string | null {
  const problem = checkKeymapText(candidate);
  if (problem) return problem;
  file.save(candidate);
  return null;
}
