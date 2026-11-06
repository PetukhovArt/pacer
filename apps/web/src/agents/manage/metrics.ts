import { t } from '../../i18n';
import { call, PacerCallError, type PacerReply } from '../pacer';

export type MetricsSnapshot = Extract<PacerReply, { Metrics: unknown }>['Metrics']['snapshot'];
export type SessionMetrics = MetricsSnapshot['sessions'][number];

export async function getMetrics(): Promise<MetricsSnapshot> {
  const reply = await call({ GetMetrics: {} });
  if (!('Metrics' in reply)) throw new PacerCallError(t('agents.errors.expectedMetrics', { reply: Object.keys(reply).join('') }));
  return reply.Metrics.snapshot;
}
