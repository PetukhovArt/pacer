import { ref, shallowRef } from 'vue';
import { invoke, isDesktop } from '../bridge';
import { settleAll } from '../files/documents';

type Build = { buildId: string; commit: string; subject: string; sourceRepo: string; builtAt: string; protocol: number };
type JobState = 'running' | 'done' | 'failed' | 'cancelled';
type Job = {
  state: JobState;
  step?: string;
  commit?: string;
  subject?: string;
  buildId?: string;
  log?: string;
  error?: string;
  upToDate?: boolean;
  tail?: string[];
};
type UpdateInfo = {
  root: string;
  running: Build | null;
  current: Build | null;
  previous: Build | null;
  sourceRepo: string | null;
  head: { commit: string; subject: string } | null;
  headError: string | null;
  daemonProtocol: number | null;
  job: Job | null;
};
type DialogMode = 'update' | 'rollback';

const POLL_MS = 1000;

export const dialogMode = shallowRef<DialogMode | null>(null);
export const info = shallowRef<UpdateInfo | null>(null);
export const busy = ref(false);
export const failure = ref<string | null>(null);
export const daemonUpdated = ref(false);

let poller: ReturnType<typeof setInterval> | undefined;

export const short = (commit: string | undefined) => (commit ?? '').slice(0, 8);

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function refresh(): Promise<void> {
  if (!isDesktop) return;
  try {
    info.value = await invoke<UpdateInfo>('app_update_info');
  } catch (error) {
    failure.value = message(error);
  }
  if (info.value?.job?.state === 'running') watchJob();
}

function watchJob(): void {
  if (poller) return;
  poller = setInterval(() => void poll(), POLL_MS);
}

async function poll(): Promise<void> {
  const job = await invoke<Job | null>('app_update_status').catch(() => null);
  if (info.value) info.value = { ...info.value, job };
  if (job?.state === 'running') return;
  clearInterval(poller);
  poller = undefined;
  await refresh();
  if (job?.state === 'done' && !job.upToDate) dialogMode.value ??= 'update';
}

async function attempt(work: () => Promise<unknown>): Promise<void> {
  busy.value = true;
  failure.value = null;
  try {
    await work();
  } catch (error) {
    failure.value = message(error);
  } finally {
    busy.value = false;
  }
  await refresh();
}

export function openUpdate(mode: DialogMode): void {
  failure.value = null;
  daemonUpdated.value = false;
  dialogMode.value = mode;
  void refresh();
}

export function closeUpdate(): void {
  dialogMode.value = null;
}

export const startBuild = () => attempt(() => invoke('app_update_start'));
export const cancelBuild = () => attempt(() => invoke('app_update_cancel'));
export const rollBack = () => attempt(() => invoke('app_rollback'));

export const updateDaemon = () =>
  attempt(async () => {
    await invoke('app_daemon_update');
    daemonUpdated.value = true;
  });

export async function restart(): Promise<void> {
  if (!(await settleAll())) return;
  await attempt(() => invoke('app_restart'));
}

export function installUpdate(): void {
  void refresh();
}
