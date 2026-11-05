import { call, PacerCallError, type PacerReply } from '../pacer';

export type MetricsSnapshot = Extract<PacerReply, { Metrics: unknown }>['Metrics']['snapshot'];
export type SessionMetrics = MetricsSnapshot['sessions'][number];

export async function getMetrics(): Promise<MetricsSnapshot> {
  const reply = await call({ GetMetrics: {} });
  if (!('Metrics' in reply)) throw new PacerCallError(`pacer ответил ${Object.keys(reply).join('')} вместо Metrics`);
  return reply.Metrics.snapshot;
}
