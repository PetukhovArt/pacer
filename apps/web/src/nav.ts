import { ref } from 'vue';

export type View = { kind: 'tasks' } | { kind: 'task'; id: string } | { kind: 'topic'; id: string };

export const view = ref<View>({ kind: 'tasks' });

export function openTask(id: string): void {
  view.value = { kind: 'task', id };
}

export function openTopic(id: string): void {
  view.value = { kind: 'topic', id };
}
