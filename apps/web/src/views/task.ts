import { computed, onScopeDispose, ref } from 'vue';
import type { FeedEvent } from '@core/client-sdk';
import { useClient, useClientState } from '@core/vue-bindings';

export function useTask(taskId: string) {
  const client = useClient();
  const state = useClientState();
  const error = ref('');
  onScopeDispose(client.openTask(taskId));

  const task = computed(() => state.value.tasks.find((t) => t.id === taskId));
  const events = computed(() => state.value.events.get(taskId) ?? []);
  const mergeRequests = computed(() => state.value.mergeRequests.filter((m) => m.taskRefs.includes(taskId)));
  const agents = computed(() => [...new Set(events.value.flatMap((e) => (e.type === 'chat.message' ? [] : [e.actorId])))]);

  function lastOf<T extends FeedEvent['type']>(actorId: string, type: T) {
    return events.value.findLast((e): e is Extract<FeedEvent, { type: T }> => e.actorId === actorId && e.type === type) ?? null;
  }

  function statusOf(actorId: string) {
    return lastOf(actorId, 'agent.status')?.status ?? 'running';
  }

  function worktreeOf(actorId: string) {
    return events.value.findLast((e): e is Extract<FeedEvent, { type: 'agent.status' }> => e.actorId === actorId && e.type === 'agent.status' && e.status === 'running')?.detail ?? '';
  }

  async function run(action: () => Promise<unknown>) {
    error.value = '';
    await action().catch((e: unknown) => (error.value = e instanceof Error ? e.message : String(e)));
  }

  return { client, state, error, task, events, mergeRequests, agents, lastOf, statusOf, worktreeOf, run };
}

export type TaskContext = ReturnType<typeof useTask>;
