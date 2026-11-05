import type { ChangeRequestSummary, PipelineStatus } from '../connectors/api';

export const PIPELINE_LABEL: Record<PipelineStatus, string> = {
  success: 'Пайплайн успешен',
  failed: 'Пайплайн упал',
  running: 'Пайплайн выполняется',
  pending: 'Пайплайн ожидает',
  canceled: 'Пайплайн отменён',
  none: 'Нет пайплайна',
};

const JOB_TONE: Record<string, PipelineStatus> = {
  success: 'success',
  passed: 'success',
  completed: 'success',
  failed: 'failed',
  failure: 'failed',
  error: 'failed',
  running: 'running',
  in_progress: 'running',
  pending: 'pending',
  created: 'pending',
  queued: 'pending',
  scheduled: 'pending',
  preparing: 'pending',
  waiting_for_resource: 'pending',
  manual: 'pending',
  canceled: 'canceled',
  cancelled: 'canceled',
  skipped: 'canceled',
};

export function jobTone(status: string): PipelineStatus {
  return JOB_TONE[status.toLowerCase()] ?? 'none';
}

export function numberLabel(item: Pick<ChangeRequestSummary, 'url' | 'number'>): string {
  return `${item.url.includes('/merge_requests/') ? '!' : '#'}${item.number}`;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function relativeTime(iso: string, now = Date.now()): string {
  const time = Date.parse(iso);
  if (Number.isNaN(time)) return '';
  const diff = Math.max(0, now - time);
  if (diff < MINUTE) return 'только что';
  if (diff < HOUR) return `${Math.floor(diff / MINUTE)} мин назад`;
  if (diff < DAY) return `${Math.floor(diff / HOUR)} ч назад`;
  if (diff < 2 * DAY) return 'вчера';
  if (diff < 30 * DAY) return `${Math.floor(diff / DAY)} дн назад`;
  return new Date(time).toLocaleDateString('ru-RU', { day: 'numeric', month: 'short', year: diff > 300 * DAY ? 'numeric' : undefined });
}

export function fullDate(iso: string): string {
  const time = Date.parse(iso);
  return Number.isNaN(time) ? '' : new Date(time).toLocaleString('ru-RU');
}

export function formatDuration(seconds: number | null): string {
  if (seconds == null) return '';
  const total = Math.round(seconds);
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return minutes > 0 ? `${minutes}м ${rest}с` : `${rest}с`;
}
