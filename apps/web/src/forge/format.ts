import type { ChangeRequestSummary, PipelineStatus } from '../connectors/api';
import { t, useI18n } from '../i18n';

const PIPELINE_KEY = {
  success: 'forge.pipeline.success',
  failed: 'forge.pipeline.failed',
  running: 'forge.pipeline.running',
  pending: 'forge.pipeline.pending',
  canceled: 'forge.pipeline.canceled',
  none: 'forge.pipeline.none',
} as const satisfies Record<PipelineStatus, string>;

export function pipelineLabel(status: PipelineStatus): string {
  return t(PIPELINE_KEY[status]);
}

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
  if (diff < MINUTE) return t('forge.time.justNow');
  if (diff < HOUR) return t('forge.time.minutesAgo', { count: Math.floor(diff / MINUTE) });
  if (diff < DAY) return t('forge.time.hoursAgo', { count: Math.floor(diff / HOUR) });
  if (diff < 2 * DAY) return t('forge.time.yesterday');
  if (diff < 30 * DAY) return t('forge.time.daysAgo', { count: Math.floor(diff / DAY) });
  return useI18n().formatDate(time, { day: 'numeric', month: 'short', year: diff > 300 * DAY ? 'numeric' : undefined });
}

export function fullDate(iso: string): string {
  const time = Date.parse(iso);
  return Number.isNaN(time) ? '' : useI18n().formatDate(time, { dateStyle: 'short', timeStyle: 'medium' });
}

export function formatDuration(seconds: number | null): string {
  if (seconds == null) return '';
  const total = Math.round(seconds);
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return minutes > 0 ? t('forge.time.durationMinutes', { minutes, seconds: rest }) : t('forge.time.durationSeconds', { seconds: rest });
}
