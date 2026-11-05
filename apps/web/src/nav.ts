import { ref } from 'vue';
import type { ChangeRequestRef } from './connectors/api';

export type View =
  | { kind: 'tasks' }
  | { kind: 'agents' }
  | { kind: 'reviews'; open?: ChangeRequestRef }
  | { kind: 'jira'; open?: { connector: string; key: string } }
  | { kind: 'confluence'; open?: { connector: string; id: string } }
  | { kind: 'settings' }
  | { kind: 'task'; id: string }
  | { kind: 'topic'; id: string };

export const view = ref<View>({ kind: 'tasks' });
export const requestedSettingsCategory = ref<string | null>(null);

export function openTask(id: string): void {
  view.value = { kind: 'task', id };
}

export function openTopic(id: string): void {
  view.value = { kind: 'topic', id };
}

export function openReview(ref: ChangeRequestRef): void {
  view.value = { kind: 'reviews', open: ref };
}

export function openSettings(category?: string): void {
  requestedSettingsCategory.value = category ?? null;
  view.value = { kind: 'settings' };
}
