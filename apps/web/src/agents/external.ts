import { computed, shallowRef, watch, type ComputedRef, type ShallowRef } from 'vue';
import { createChannel, hasBackend, invoke } from '../bridge';
import { t } from '../i18n';
import { useSettings } from '../settings/store';
import { getMetrics } from './manage/metrics';
import { listOrphans, resumeOrphan } from './manage/orphans';
import { createTerminal, describeError, entitySessionRef, PacerCallError, sendInput, sessionKey, usePacer, worktreesOfProject, type Entities, type SessionRef, type SessionStatus } from './pacer';

export type Harness = 'claude' | 'codex';
type ExternalStatus = 'working' | 'waiting' | 'idle';
export type ExternalSession = {
  harness: Harness;
  id: string;
  cwd: string;
  title: string;
  branch: string | null;
  updated_at: number;
  status: ExternalStatus;
  running: boolean;
  host: string | null;
  in_pacer: boolean;
  pacer_pids: number[];
  messages: number;
};
export type TranscriptMessage = { key: string; role: 'user' | 'assistant'; text: string; tool: string | null; at: string | null };
export type ExternalTarget = { projectId: string; projectName: string; worktreeId: string; branch: string; exact: boolean };
export type Placement = { kind: 'worktree'; projectId: string; worktreeId: string } | { kind: 'project'; projectId: string } | { kind: 'loose'; folder: string };

type Store = {
  sessions: ComputedRef<readonly ExternalSession[]>;
  linked: ComputedRef<ReadonlyMap<string, ExternalSession>>;
  terminalOf: ComputedRef<ReadonlyMap<string, string>>;
  byKey: ComputedRef<ReadonlyMap<string, ExternalSession>>;
  anyByKey: ComputedRef<ReadonlyMap<string, ExternalSession>>;
  error: ShallowRef<string | null>;
};

const EXTERNAL_PREFIX = 'ext:';
const TERMINAL_NAME_CHARS = 40;
const LISTED_TIMEOUT_MS = 5000;
const EARLIER_MS = 24 * 3_600_000;
const RECENT_CLOSED = 3;
const METRICS_DEBOUNCE_MS = 300;

export const externalKey = (session: Pick<ExternalSession, 'harness' | 'id'>) => `${EXTERNAL_PREFIX}${session.harness}:${session.id}`;
export const isExternalPanel = (id: string) => id.startsWith(EXTERNAL_PREFIX);

export function resumeCommand(session: Pick<ExternalSession, 'harness' | 'id'>): string {
  return session.harness === 'claude' ? `claude --resume ${session.id}` : `codex resume ${session.id}`;
}

export function externalDot(session: ExternalSession): SessionStatus {
  if (!session.running) return 'closed';
  if (session.status === 'working') return 'running';
  if (session.status === 'waiting') return 'needs_feedback';
  return 'finished';
}

export function externalStatusLabel(session: ExternalSession): string {
  return session.running ? t(`workbench.external.status.${session.status}`) : t('workbench.external.status.closed');
}

export function externalOrigin(session: ExternalSession): string {
  if (session.in_pacer) return t('workbench.external.inTerminal');
  return session.host ? t('workbench.external.startedIn', { host: session.host }) : t('workbench.external.startedOutside');
}

export function splitEarlier(sessions: readonly ExternalSession[], now: number): { recent: ExternalSession[]; earlier: ExternalSession[] } {
  const recent: ExternalSession[] = [];
  const earlier: ExternalSession[] = [];
  let closed = 0;
  for (const session of sessions) {
    if (session.running) recent.push(session);
    else if (closed < RECENT_CLOSED && now - session.updated_at <= EARLIER_MS) {
      closed += 1;
      recent.push(session);
    } else earlier.push(session);
  }
  return { recent, earlier };
}

const normalizeDir = (path: string) => path.replaceAll('\\', '/').replace(/^\/\/\?\//, '').replace(/\/+$/, '').toLowerCase();

const within = (path: string, root: string) => path === root || path.startsWith(`${root}/`);
const parentOf = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf('/')));
const baseOf = (path: string) => path.slice(path.lastIndexOf('/') + 1);

export function placeExternal(cwd: string, entities: Entities | undefined): Placement {
  const dir = normalizeDir(cwd);
  if (!entities) return { kind: 'loose', folder: dir };
  let best: { projectId: string; worktreeId: string; length: number } | null = null;
  for (const worktree of entities.worktrees.values()) {
    const root = normalizeDir(worktree.path);
    if (within(dir, root) && (!best || root.length > best.length)) best = { projectId: worktree.project_id, worktreeId: worktree.id, length: root.length };
  }
  if (best) return { kind: 'worktree', projectId: best.projectId, worktreeId: best.worktreeId };
  for (const project of entities.projects.values()) {
    const repo = normalizeDir(project.repo_path);
    const name = baseOf(repo);
    const homes = [repo, `${parentOf(repo)}/${name}-worktrees`];
    if (homes.some((home) => within(dir, home))) return { kind: 'project', projectId: project.id };
    for (const worktree of worktreesOfProject(entities, project.id)) {
      if (worktree.is_main) continue;
      const parent = parentOf(normalizeDir(worktree.path));
      if (!parent || !within(dir, parent) || dir === parent) continue;
      const segment = dir.slice(parent.length + 1).split('/')[0] ?? '';
      if (segment.startsWith(`${name}-`)) return { kind: 'project', projectId: project.id };
    }
  }
  return { kind: 'loose', folder: dir };
}

export function externalTarget(session: ExternalSession, entities: Entities | undefined): ExternalTarget | null {
  if (!entities) return null;
  const placement = placeExternal(session.cwd, entities);
  if (placement.kind === 'loose') return null;
  const project = entities.projects.get(placement.projectId);
  const worktrees = worktreesOfProject(entities, placement.projectId);
  const worktree = placement.kind === 'worktree' ? entities.worktrees.get(placement.worktreeId) : (worktrees.find((candidate) => candidate.is_main) ?? worktrees[0]);
  if (!project || !worktree) return null;
  return { projectId: project.id, projectName: project.name, worktreeId: worktree.id, branch: worktree.branch, exact: placement.kind === 'worktree' };
}

export function readTranscript(session: Pick<ExternalSession, 'harness' | 'id'>, tail = 200): Promise<TranscriptMessage[]> {
  return invoke<TranscriptMessage[]>('agent_session_transcript', { harness: session.harness, id: session.id, tail });
}

export async function continueInPacer(session: ExternalSession, target: ExternalTarget): Promise<SessionRef> {
  if (session.harness === 'claude') {
    const orphans = await listOrphans(target.projectId).catch(() => []);
    if (orphans.some((orphan) => orphan.session_id === session.id)) return { Agent: await resumeOrphan(session.id, target.worktreeId) };
  }
  const terminal = entitySessionRef(await createTerminal(target.worktreeId, session.title.slice(0, TERMINAL_NAME_CHARS)));
  if (!terminal) throw new PacerCallError(t('workbench.external.noTerminal'));
  await whenListed(terminal);
  await sendInput(terminal, `${resumeCommand(session)}\r`);
  return terminal;
}

function whenListed(session: SessionRef): Promise<void> {
  const pacer = usePacer();
  const key = sessionKey(session);
  if (!pacer || pacer.sessions.value.has(key)) return Promise.resolve();
  return new Promise((resolve) => {
    const finish = () => {
      clearTimeout(timer);
      stop();
      resolve();
    };
    const timer = setTimeout(finish, LISTED_TIMEOUT_MS);
    const stop = watch(
      () => pacer.sessions.value.has(key),
      (listed) => {
        if (listed) finish();
      },
    );
  });
}

function terminalByName(session: ExternalSession, entities: Entities): string | null {
  const dir = normalizeDir(session.cwd);
  const name = session.title.slice(0, TERMINAL_NAME_CHARS);
  for (const terminal of entities.terminals.values()) {
    const worktree = entities.worktrees.get(terminal.worktree_id);
    if (terminal.name === name && worktree && within(dir, normalizeDir(worktree.path))) return sessionKey({ Terminal: terminal.id });
  }
  return null;
}

let store: Store | null = null;

function createStore(): Store {
  const pacer = usePacer();
  const { values } = useSettings();
  const raw = shallowRef<readonly ExternalSession[]>([]);
  const error = shallowRef<string | null>(null);
  const days = computed(() => values.value['sessions.externalDays']);

  watch(
    days,
    (value) => {
      raw.value = [];
      error.value = null;
      const onEvent = createChannel<{ sessions: ExternalSession[] }>((event) => {
        if (days.value === value) raw.value = event.sessions;
      });
      invoke<void>('agent_session_watch', { sinceDays: value, onEvent }).catch((e: unknown) => {
        error.value = describeError(e);
      });
    },
    { immediate: true },
  );

  const pacerIds = computed(() => {
    const ids = new Set<string>();
    for (const agent of pacer?.entities.value.agents.values() ?? []) if (agent.session_id) ids.add(agent.session_id);
    return ids;
  });
  const pidOwners = shallowRef<ReadonlyMap<number, string> | null>(null);
  const pacerHosted = computed(() => raw.value.filter((session) => session.in_pacer && !pacerIds.value.has(session.id)));
  const refreshSignature = computed(() => `${pacerHosted.value.map((session) => `${externalKey(session)}@${session.pacer_pids.join('.')}`).join(',')}|${[...(pacer?.sessions.value.keys() ?? [])].join(',')}`);
  let metricsRun = 0;
  let metricsTimer: ReturnType<typeof setTimeout> | undefined;

  watch(
    refreshSignature,
    () => {
      clearTimeout(metricsTimer);
      if (!pacerHosted.value.length) return;
      metricsTimer = setTimeout(() => {
        const run = ++metricsRun;
        getMetrics().then(
          (snapshot) => {
            if (run === metricsRun) pidOwners.value = new Map(snapshot.sessions.map((entry) => [entry.pid, sessionKey(entry.session)]));
          },
          () => {
            if (run === metricsRun) pidOwners.value = new Map();
          },
        );
      }, METRICS_DEBOUNCE_MS);
    },
    { immediate: true },
  );

  const hostKey = computed(() => {
    const keys = new Map<string, string>();
    const entities = pacer?.entities.value;
    const owners = pidOwners.value;
    for (const session of pacerHosted.value) {
      const key = owners ? session.pacer_pids.map((pid) => owners.get(pid)).find((owner) => owner !== undefined) : undefined;
      const resolved = key ?? (entities ? terminalByName(session, entities) : null);
      if (resolved) keys.set(externalKey(session), resolved);
    }
    return keys;
  });
  const linked = computed(() => {
    const map = new Map<string, ExternalSession>();
    for (const session of pacerHosted.value) {
      const key = hostKey.value.get(externalKey(session));
      if (!key?.startsWith('terminal:')) continue;
      const current = map.get(key);
      if (!current || current.updated_at < session.updated_at) map.set(key, session);
    }
    return map;
  });
  const terminalOf = computed(() => new Map([...linked.value].map(([terminal, session]) => [externalKey(session), terminal])));
  const sessions = computed(() =>
    raw.value.filter((session) => {
      if (pacerIds.value.has(session.id)) return false;
      if (!session.in_pacer) return true;
      if (hostKey.value.has(externalKey(session))) return false;
      return pidOwners.value !== null;
    }),
  );
  const byKey = computed(() => new Map(sessions.value.map((session) => [externalKey(session), session])));
  const anyByKey = computed(() => new Map(raw.value.map((session) => [externalKey(session), session])));
  return { sessions, linked, terminalOf, byKey, anyByKey, error };
}

export function useExternalSessions(): Store | null {
  if (!hasBackend) return null;
  store ??= createStore();
  return store;
}
