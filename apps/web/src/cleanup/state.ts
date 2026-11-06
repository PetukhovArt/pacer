import { computed, shallowRef, watch } from 'vue';
import { archiveAgent, acknowledged, closeTerminal, describeError, useBerth, type Entities } from '../agents/berth';
import { invoke, isDesktop } from '../bridge';
import { useSettings } from '../settings/store';

type Category = 'safe' | 'review' | 'changes' | 'keep';
type Reason = 'request_merged' | 'request_closed' | 'merged' | 'branch_gone' | 'stale' | 'missing' | 'active';
type BranchRequest = { connector: string; project: string; number: number; kind: 'github' | 'gitlab'; state: 'open' | 'merged' | 'closed' };
type WorktreeScan = {
  id: string;
  path: string;
  branch: string | null;
  category: Category;
  reason: Reason;
  live: boolean;
  changes: number;
  untracked: number;
  inProgress: boolean;
  ahead: number | null;
  idleDays: number | null;
  request: BranchRequest | null;
};
type Orphan = { path: string; kind: 'stale_git' | 'leftover' };
type ProjectScan = { id: string; repo: string; base: string | null; fetched: boolean; error: string | null; worktrees: WorktreeScan[]; orphans: Orphan[] };
type Stored = { at: number; projects: ProjectScan[] };
type LinkCheck = { link: string; target: string | null; before: number | null; after: number | null };
type Unlinked = { links: LinkCheck[]; removed: boolean };

type CandidateCategory = 'safe' | 'review' | 'changes' | 'orphan';
export type Candidate = {
  key: string;
  projectId: string;
  projectName: string;
  repo: string;
  base: string | null;
  category: CandidateCategory;
  path: string;
  name: string;
  worktreeId: string | null;
  scan: WorktreeScan | null;
  orphan: Orphan | null;
};
export type BadgeKind = 'merged' | 'closed' | 'stale' | 'gone';
export type RemoveResult = { key: string; name: string; error: string | null; links: LinkCheck[] };

const STORE_KEY = 'berth.cleanup.scan';
const DAY_MS = 24 * 60 * 60 * 1000;
const CHECK_EVERY_MS = 60 * 60 * 1000;
const STOP_WAIT_MS = 8000;
const CATEGORY_ORDER: readonly CandidateCategory[] = ['safe', 'orphan', 'changes', 'review'];

function loadStored(): Stored | null {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(STORE_KEY) ?? 'null');
    if (typeof value === 'object' && value !== null && 'at' in value && 'projects' in value && Array.isArray(value.projects) && typeof value.at === 'number') return value as Stored;
  } catch {
    return null;
  }
  return null;
}

export const cleanupOpen = shallowRef(false);
export const scanned = shallowRef<Stored | null>(loadStored());
export const scanning = shallowRef(false);
export const scanError = shallowRef<string | null>(null);
export const sizes = shallowRef<ReadonlyMap<string, number>>(new Map());

export function openCleanup(): void {
  cleanupOpen.value = true;
}

function folderName(path: string): string {
  return path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? path;
}

function scanInput(entities: Entities) {
  return [...entities.projects.values()].map((project) => ({
    id: project.id,
    repo: project.repo_path,
    worktrees: [...entities.worktrees.values()]
      .filter((worktree) => worktree.project_id === project.id && !worktree.is_main)
      .map((worktree) => {
        const agents = [...entities.agents.values()].filter((agent) => agent.worktree_id === worktree.id);
        const terminals = [...entities.terminals.values()].filter((terminal) => terminal.worktree_id === worktree.id);
        const live = agents.some((agent) => agent.alive) || terminals.some((terminal) => terminal.alive);
        const lastSessionMs = agents.reduce((latest, agent) => Math.max(latest, agent.status_changed_at), 0);
        return { id: worktree.id, path: worktree.path, live, lastSessionMs };
      }),
  }));
}

function candidatesOf(stored: Stored | null, entities: Entities): Candidate[] {
  if (!stored) return [];
  const out: Candidate[] = [];
  for (const project of stored.projects) {
    const known = entities.projects.get(project.id);
    if (!known) continue;
    for (const scan of project.worktrees) {
      const worktree = entities.worktrees.get(scan.id);
      if (!worktree || worktree.is_main || scan.category === 'keep') continue;
      out.push({
        key: `w:${scan.id}`,
        projectId: project.id,
        projectName: known.name,
        repo: project.repo,
        base: project.base,
        category: scan.category,
        path: scan.path,
        name: scan.branch ?? worktree.branch,
        worktreeId: scan.id,
        scan,
        orphan: null,
      });
    }
    for (const orphan of project.orphans) {
      out.push({ key: `o:${orphan.path}`, projectId: project.id, projectName: known.name, repo: project.repo, base: project.base, category: 'orphan', path: orphan.path, name: folderName(orphan.path), worktreeId: null, scan: null, orphan });
    }
  }
  return out.sort((a, b) => a.projectName.localeCompare(b.projectName) || CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category) || a.name.localeCompare(b.name));
}

const berth = useBerth();
const EMPTY_ENTITIES: Entities = { workspaces: new Map(), projects: new Map(), worktrees: new Map(), agents: new Map(), terminals: new Map() };
const entities = computed(() => berth?.entities.value ?? EMPTY_ENTITIES);

export const candidates = computed(() => candidatesOf(scanned.value, entities.value));
export const cleanupCount = computed(() => candidates.value.filter((candidate) => candidate.category === 'safe' || candidate.category === 'orphan').length);

export const badges = computed(() => {
  const out = new Map<string, BadgeKind>();
  for (const project of scanned.value?.projects ?? []) {
    for (const scan of project.worktrees) {
      const kind: BadgeKind | null =
        scan.reason === 'request_merged' || scan.reason === 'merged'
          ? 'merged'
          : scan.reason === 'request_closed'
            ? 'closed'
            : scan.reason === 'branch_gone'
              ? 'gone'
              : scan.reason === 'stale'
                ? 'stale'
                : null;
      if (kind) out.set(scan.id, kind);
    }
  }
  return out;
});

async function measure(list: readonly Candidate[]): Promise<void> {
  const next = new Map<string, number>();
  await Promise.all(
    list.map(async (candidate) => {
      if (candidate.scan?.reason === 'missing') return;
      try {
        const [size] = await invoke<number[]>('cleanup_sizes', { paths: [candidate.path] });
        if (size !== undefined) next.set(candidate.path, size);
      } catch {
        return;
      }
    }),
  );
  sizes.value = next;
}

export async function scanNow(): Promise<void> {
  if (!isDesktop || !berth || scanning.value) return;
  scanning.value = true;
  scanError.value = null;
  try {
    const { values } = useSettings();
    const staleDays = Math.max(1, Math.round(values.value['cleanup.staleDays']));
    const projects = await invoke<ProjectScan[]>('cleanup_scan', { projects: scanInput(entities.value), staleDays });
    scanned.value = { at: Date.now(), projects };
    localStorage.setItem(STORE_KEY, JSON.stringify(scanned.value));
    if (cleanupOpen.value) void measure(candidates.value);
  } catch (error) {
    scanError.value = describeError(error);
  } finally {
    scanning.value = false;
  }
}

export function measureSizes(): void {
  void measure(candidates.value);
}

function liveIn(worktreeId: string): boolean {
  const current = entities.value;
  return [...current.agents.values()].some((agent) => agent.worktree_id === worktreeId && agent.alive) || [...current.terminals.values()].some((terminal) => terminal.worktree_id === worktreeId && terminal.alive);
}

function waitStopped(worktreeId: string): Promise<boolean> {
  if (!liveIn(worktreeId)) return Promise.resolve(true);
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      stop();
      resolve(false);
    }, STOP_WAIT_MS);
    const stop = watch(entities, () => {
      if (liveIn(worktreeId)) return;
      clearTimeout(timer);
      stop();
      resolve(true);
    });
  });
}

async function stopSessions(worktreeId: string): Promise<void> {
  const current = entities.value;
  for (const agent of current.agents.values()) {
    if (agent.worktree_id === worktreeId && agent.alive) await archiveAgent(agent.id);
  }
  for (const terminal of current.terminals.values()) {
    if (terminal.worktree_id === worktreeId && terminal.alive) await closeTerminal(terminal.id);
  }
  await waitStopped(worktreeId);
}

async function removeOne(candidate: Candidate, deleteBranch: boolean): Promise<LinkCheck[]> {
  if (candidate.worktreeId) {
    await stopSessions(candidate.worktreeId);
    const unlinked = await invoke<Unlinked>('cleanup_unlink', { repo: candidate.repo, path: candidate.path, remove: false });
    await acknowledged({ DeleteWorktree: { id: candidate.worktreeId, force: true } });
    await invoke<boolean>('cleanup_finish', { repo: candidate.repo, branch: candidate.scan?.branch ?? null, deleteBranch });
    return unlinked.links;
  }
  const unlinked = await invoke<Unlinked>('cleanup_unlink', { repo: candidate.repo, path: candidate.path, remove: true });
  await invoke<boolean>('cleanup_finish', { repo: candidate.repo, branch: null, deleteBranch: false });
  return unlinked.links;
}

export async function removeCandidates(list: readonly Candidate[], deleteBranch: boolean, progress: (name: string) => void): Promise<RemoveResult[]> {
  const results: RemoveResult[] = [];
  for (const candidate of list) {
    progress(candidate.name);
    try {
      const links = await removeOne(candidate, deleteBranch);
      results.push({ key: candidate.key, name: candidate.name, error: null, links });
    } catch (error) {
      results.push({ key: candidate.key, name: candidate.name, error: describeError(error), links: [] });
    }
  }
  await scanNow();
  return results;
}

export function cleanupDiff(path: string): Promise<string> {
  return invoke<string>('cleanup_diff', { path });
}

let installed = false;

export function installCleanup(): void {
  if (installed || !isDesktop || !berth) return;
  installed = true;
  const due = () => !scanned.value || Date.now() - scanned.value.at >= DAY_MS;
  const check = () => {
    if (berth.connection.value.state === 'connected' && entities.value.projects.size > 0 && due()) void scanNow();
  };
  watch(() => [berth.connection.value.state, entities.value.projects.size] as const, check, { immediate: true });
  setInterval(check, CHECK_EVERY_MS);
  watch(
    cleanupCount,
    (count) => {
      void invoke('tray_cleanup', { count }).catch(() => undefined);
    },
    { immediate: true },
  );
}
