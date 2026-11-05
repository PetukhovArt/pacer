import { shallowRef } from 'vue';
import type { PlainKey } from '../settings/translate';
import { readContext } from './context';
import { parseCondition, type Condition, type Context } from './when';

export type ActionDefinition = {
  id: string;
  title: PlainKey;
  category: PlainKey;
  run: () => unknown;
  when?: string;
};

export type Action = Readonly<ActionDefinition> & { readonly condition: Condition | null };

const RECENT_KEY = 'veyra.commands.recent';
const RECENT_LIMIT = 12;
const PALETTE_ID = 'palette.open';

const actions = new Map<string, Action>();

function loadRecent(): string[] {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
    return Array.isArray(stored) ? stored.filter((id): id is string => typeof id === 'string') : [];
  } catch {
    return [];
  }
}

export const recentActionIds = shallowRef<readonly string[]>(loadRecent());

export function defineAction(definition: ActionDefinition): Action {
  if (actions.has(definition.id)) throw new Error(`action ${definition.id} is already defined`);
  const action: Action = { ...definition, condition: definition.when ? parseCondition(definition.when) : null };
  actions.set(action.id, action);
  return action;
}

export function listActions(): readonly Action[] {
  return [...actions.values()];
}

export function getAction(id: string): Action | undefined {
  return actions.get(id);
}

export function isAvailable(action: Action, context: Context): boolean {
  return !action.condition || action.condition(context);
}

function remember(id: string): void {
  recentActionIds.value = [id, ...recentActionIds.value.filter((entry) => entry !== id)].slice(0, RECENT_LIMIT);
  localStorage.setItem(RECENT_KEY, JSON.stringify(recentActionIds.value));
}

export function runAction(id: string, context: Context = readContext(null)): boolean {
  const action = actions.get(id);
  if (!action || !isAvailable(action, context)) return false;
  if (id !== PALETTE_ID) remember(id);
  void action.run();
  return true;
}
