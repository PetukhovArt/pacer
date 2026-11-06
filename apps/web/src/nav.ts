import { ref } from 'vue';
import type { ChangeRequestRef } from './connectors/api';
import { withViewTransition } from './motion';

export type View =
  | { kind: 'tasks' }
  | { kind: 'agents' }
  | { kind: 'reviews'; open?: ChangeRequestRef }
  | { kind: 'jira'; open?: { connector: string; key: string } }
  | { kind: 'confluence'; open?: { connector: string; id: string } }
  | { kind: 'settings' }
  | { kind: 'task'; id: string }
  | { kind: 'topic'; id: string };

export const view = ref<View>({ kind: 'agents' });
export const requestedSettingsCategory = ref<string | null>(null);

export function navigate(next: View): Promise<void> {
  return withViewTransition(() => {
    view.value = next;
  });
}

export function openTopic(id: string): void {
  void navigate({ kind: 'topic', id });
}

export function openSettings(category?: string): void {
  requestedSettingsCategory.value = category ?? null;
  if (view.value.kind !== 'settings') void navigate({ kind: 'settings' });
}
