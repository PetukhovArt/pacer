import { computed, shallowRef } from 'vue';
import type { SessionStatus } from '../agents/pacer';
import { useSettings } from '../settings/store';
import type { HotStatus } from './hot';

const { values } = useSettings();

export const hotEnabled = computed(() => (values.value as Record<string, unknown>)['workbench.hot'] !== 'off');

export const hotOverviewOpen = shallowRef(false);

export function toggleHotOverview(): void {
  hotOverviewOpen.value = !hotOverviewOpen.value;
}

export function hotDot(status: HotStatus): SessionStatus | undefined {
  if (status === 'waiting') return 'needs_feedback';
  if (status === 'working') return 'running';
  if (status === 'error') return 'terminated';
  return undefined;
}
