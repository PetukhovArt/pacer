import type { DockviewApi } from 'dockview-core';
import type { Task } from '@veyra/protocol';

export type TaskPanelParams = { taskId: string };

export function openTask(api: DockviewApi, task: Task): void {
  const id = `task:${task.id}`;
  const existing = api.getPanel(id);
  if (existing) return existing.api.setActive();
  const params: TaskPanelParams = { taskId: task.id };
  const position = api.getPanel('tasks') ? { direction: 'right' as const, referencePanel: 'tasks' } : undefined;
  api.addPanel({ id, component: 'task', title: task.externalRef, params, position });
}
