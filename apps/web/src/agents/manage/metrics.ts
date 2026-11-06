import { t } from '../../i18n';
import { call, BerthCallError, type BerthReply } from '../berth';

export type MetricsSnapshot = Extract<BerthReply, { Metrics: unknown }>['Metrics']['snapshot'];
export type SessionMetrics = MetricsSnapshot['sessions'][number];

export async function getMetrics(): Promise<MetricsSnapshot> {
  const reply = await call({ GetMetrics: {} });
  if (!('Metrics' in reply)) throw new BerthCallError(t('agents.errors.expectedMetrics', { reply: Object.keys(reply).join('') }));
  return reply.Metrics.snapshot;
}
