import { computed, ref } from 'vue';
import type { VeyraEvent } from '@veyra/protocol';
import { useClient, useClientState } from '@core/vue-bindings';

export function useTask(taskId: string) {
  const client = useClient();
  const state = useClientState();
  const error = ref('');

  const task = computed(() => state.value.tasks.find((t) => t.id === taskId));
  const field = computed(() => state.value.fields.get(taskId));
  const events = computed(() => state.value.events.get(taskId) ?? []);
  const projects = computed(() => state.value.workspaces.find((w) => w.id === state.value.workspaceId)?.projects ?? []);
  const mergeRequests = computed(() => (task.value ? state.value.mergeRequests.filter((m) => m.taskRefs.includes(task.value!.externalRef)) : []));
  const inTauri = '__TAURI_INTERNALS__' in window;

  function projectName(id: string | null | undefined) {
    return projects.value.find((p) => p.id === id)?.name ?? id ?? '';
  }

  function agentOf(actorId: string) {
    const actor = state.value.actors.get(actorId);
    return actor?.kind === 'agent' ? actor : null;
  }

  function statusOf(actorId: string) {
    const actor = state.value.actors.get(actorId);
    return actor?.kind === 'agent' ? actor.status : state.value.presence.get(actorId) ? 'online' : 'offline';
  }

  function lastOf<T extends VeyraEvent['type']>(actorId: string, type: T): Extract<VeyraEvent, { type: T }> | null {
    const list = events.value;
    for (let i = list.length - 1; i >= 0; i--) {
      const e = list[i]!;
      if (e.actorId === actorId && e.type === type) return e as Extract<VeyraEvent, { type: T }>;
    }
    return null;
  }

  function worktreeOf(actorId: string) {
    const running = events.value.filter((e) => e.actorId === actorId && e.type === 'agent.status' && e.status === 'running');
    const last = running[running.length - 1];
    return last?.type === 'agent.status' ? (last.detail ?? '') : '';
  }

  async function run(action: () => Promise<unknown>) {
    error.value = '';
    await action().catch((e: unknown) => (error.value = (e as Error).message));
  }

  function openMergeRequest(event: MouseEvent, id: string) {
    if (!inTauri) return;
    event.preventDefault();
    void run(() => client.openMergeRequest(id));
  }

  return { client, state, error, task, field, events, projects, mergeRequests, inTauri, projectName, agentOf, statusOf, lastOf, worktreeOf, run, openMergeRequest };
}

export type TaskContext = ReturnType<typeof useTask>;
