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
  const keyOf = (e: FeedEvent) => (e.type === 'chat.message' ? null : (e.job?.id ?? e.actorId));
  const agents = computed(() => [...new Set(events.value.flatMap((e) => keyOf(e) ?? []))]);

  function agentOf(key: string) {
    const first = events.value.find((e) => keyOf(e) === key);
    return first && first.type !== 'chat.message' ? { actorId: first.actorId, job: first.job } : null;
  }

  function lastOf<T extends FeedEvent['type']>(key: string, type: T) {
    return events.value.findLast((e): e is Extract<FeedEvent, { type: T }> => keyOf(e) === key && e.type === type) ?? null;
  }

  function statusOf(key: string) {
    return lastOf(key, 'agent.status')?.status ?? 'running';
  }

  function worktreeOf(key: string) {
    return events.value.findLast((e): e is Extract<FeedEvent, { type: 'agent.status' }> => keyOf(e) === key && e.type === 'agent.status' && e.status === 'running')?.detail ?? '';
  }

  async function run(action: () => Promise<unknown>) {
    error.value = '';
    await action().catch((e: unknown) => (error.value = e instanceof Error ? e.message : String(e)));
  }

  return { client, state, error, task, events, mergeRequests, agents, agentOf, lastOf, statusOf, worktreeOf, run };
}

export type TaskContext = ReturnType<typeof useTask>;
