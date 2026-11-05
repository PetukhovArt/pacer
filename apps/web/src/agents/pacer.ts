import { Channel, invoke } from '@tauri-apps/api/core';
import { computed, shallowRef, type ComputedRef, type ShallowRef } from 'vue';

type AgentStatus = 'fresh' | 'running' | 'finished' | 'needs_feedback' | 'terminated' | 'disconnected';
export type SessionRef = { Agent: string } | { Terminal: string };
type ConnectionState = 'connecting' | 'connected' | 'offline' | 'incompatible';
type Connection = { state: ConnectionState; message: string | null };

type Workspace = { id: string; name: string };
type Project = { id: string; name: string; workspace_id: string; sort_order: number };
type Worktree = { id: string; project_id: string; branch: string; is_main: boolean; sort_order: number };
type Agent = { id: string; worktree_id: string; name: string; status: AgentStatus; archived: boolean; unseen: boolean; sort_order: number };
type Terminal = { id: string; worktree_id: string; name: string; sort_order: number; alive: boolean };

type Entity = { Workspace: Workspace } | { Project: Project } | { Worktree: Worktree } | { Agent: Agent } | { Terminal: Terminal } | { Link: unknown };
type EntityId = { Workspace: string } | { Project: string } | { Worktree: string } | { Agent: string } | { Terminal: string } | { Link: string };

type WatchEvent =
  | ({ type: 'connection' } & Connection)
  | { type: 'snapshot'; workspaces: Workspace[]; projects: Project[]; worktrees: Worktree[]; agents: Agent[]; terminals: Terminal[] }
  | { type: 'upserted'; entity: Entity }
  | { type: 'removed'; id: EntityId }
  | { type: 'status'; agent: string; status: AgentStatus; changed_at: number; unseen: boolean }
  | { type: 'error'; message: string };

export type SessionEvent = { type: 'reset'; data: string } | { type: 'output'; data: string } | { type: 'exited'; exit_code: number | null };

export type SessionStatus = AgentStatus | 'shell' | 'closed';
export type SessionNode = { key: string; id: string; ref: SessionRef; name: string; status: SessionStatus; unseen: boolean };
type WorktreeNode = { id: string; branch: string; sessions: SessionNode[] };
type ProjectNode = { id: string; name: string; worktrees: WorktreeNode[] };
type WorkspaceNode = { id: string; name: string; projects: ProjectNode[] };

type PacerStore = {
  connection: ShallowRef<Connection>;
  error: ShallowRef<string | null>;
  tree: ComputedRef<WorkspaceNode[]>;
  sessions: ComputedRef<ReadonlyMap<string, SessionNode>>;
};

type Entities = {
  workspaces: ReadonlyMap<string, Workspace>;
  projects: ReadonlyMap<string, Project>;
  worktrees: ReadonlyMap<string, Worktree>;
  agents: ReadonlyMap<string, Agent>;
  terminals: ReadonlyMap<string, Terminal>;
};

const byId = <T extends { id: string }>(items: T[]): ReadonlyMap<string, T> => new Map(items.map((item) => [item.id, item]));
const put = <T extends { id: string }>(map: ReadonlyMap<string, T>, item: T): ReadonlyMap<string, T> => new Map(map).set(item.id, item);
const drop = <T>(map: ReadonlyMap<string, T>, id: string): ReadonlyMap<string, T> => {
  const next = new Map(map);
  next.delete(id);
  return next;
};
const bySortOrder = (a: { sort_order: number }, b: { sort_order: number }) => a.sort_order - b.sort_order;

const EMPTY: Entities = { workspaces: new Map(), projects: new Map(), worktrees: new Map(), agents: new Map(), terminals: new Map() };

function upsert(entities: Entities, entity: Entity): Entities {
  if ('Workspace' in entity) return { ...entities, workspaces: put(entities.workspaces, entity.Workspace) };
  if ('Project' in entity) return { ...entities, projects: put(entities.projects, entity.Project) };
  if ('Worktree' in entity) return { ...entities, worktrees: put(entities.worktrees, entity.Worktree) };
  if ('Agent' in entity) return { ...entities, agents: put(entities.agents, entity.Agent) };
  if ('Terminal' in entity) return { ...entities, terminals: put(entities.terminals, entity.Terminal) };
  return entities;
}

function remove(entities: Entities, id: EntityId): Entities {
  if ('Workspace' in id) return { ...entities, workspaces: drop(entities.workspaces, id.Workspace) };
  if ('Project' in id) return { ...entities, projects: drop(entities.projects, id.Project) };
  if ('Worktree' in id) return { ...entities, worktrees: drop(entities.worktrees, id.Worktree) };
  if ('Agent' in id) return { ...entities, agents: drop(entities.agents, id.Agent) };
  if ('Terminal' in id) return { ...entities, terminals: drop(entities.terminals, id.Terminal) };
  return entities;
}

function setStatus(entities: Entities, id: string, status: AgentStatus, unseen: boolean): Entities {
  const agent = entities.agents.get(id);
  if (!agent) return entities;
  return { ...entities, agents: put(entities.agents, { ...agent, status, unseen }) };
}

function sessionKey(session: SessionRef): string {
  return 'Agent' in session ? `agent:${session.Agent}` : `terminal:${session.Terminal}`;
}

export function sessionId(session: SessionRef): string {
  return 'Agent' in session ? session.Agent : session.Terminal;
}

export function isSessionRef(value: unknown): value is SessionRef {
  if (typeof value !== 'object' || value === null) return false;
  const id = 'Agent' in value ? value.Agent : 'Terminal' in value ? value.Terminal : undefined;
  return typeof id === 'string';
}

function agentNode(agent: Agent): SessionNode {
  const ref = { Agent: agent.id };
  return { key: sessionKey(ref), id: agent.id, ref, name: agent.name, status: agent.status, unseen: agent.unseen };
}

function terminalNode(terminal: Terminal): SessionNode {
  const ref = { Terminal: terminal.id };
  return { key: sessionKey(ref), id: terminal.id, ref, name: terminal.name, status: terminal.alive ? 'shell' : 'closed', unseen: false };
}

function buildTree(entities: Entities): WorkspaceNode[] {
  const sessionsOf = (worktree: string) => [
    ...[...entities.agents.values()].filter((a) => a.worktree_id === worktree && !a.archived).sort(bySortOrder).map(agentNode),
    ...[...entities.terminals.values()].filter((t) => t.worktree_id === worktree).sort(bySortOrder).map(terminalNode),
  ];
  const worktreesOf = (project: string) =>
    [...entities.worktrees.values()]
      .filter((w) => w.project_id === project)
      .sort((a, b) => Number(b.is_main) - Number(a.is_main) || bySortOrder(a, b))
      .map((w) => ({ id: w.id, branch: w.branch, sessions: sessionsOf(w.id) }))
      .filter((w) => w.sessions.length > 0);
  const projectsOf = (workspace: string) =>
    [...entities.projects.values()]
      .filter((p) => p.workspace_id === workspace)
      .sort(bySortOrder)
      .map((p) => ({ id: p.id, name: p.name, worktrees: worktreesOf(p.id) }))
      .filter((p) => p.worktrees.length > 0);
  return [...entities.workspaces.values()].map((w) => ({ id: w.id, name: w.name, projects: projectsOf(w.id) }));
}

let store: PacerStore | null = null;
let watching: Promise<void> = Promise.resolve();

function createStore(): PacerStore {
  const entities = shallowRef<Entities>(EMPTY);
  const connection = shallowRef<Connection>({ state: 'connecting', message: null });
  const error = shallowRef<string | null>(null);

  function receive(event: WatchEvent): void {
    switch (event.type) {
      case 'connection':
        connection.value = { state: event.state, message: event.message };
        error.value = null;
        return;
      case 'snapshot':
        entities.value = {
          workspaces: byId(event.workspaces),
          projects: byId(event.projects),
          worktrees: byId(event.worktrees),
          agents: byId(event.agents),
          terminals: byId(event.terminals),
        };
        return;
      case 'upserted':
        entities.value = upsert(entities.value, event.entity);
        return;
      case 'removed':
        entities.value = remove(entities.value, event.id);
        return;
      case 'status':
        entities.value = setStatus(entities.value, event.agent, event.status, event.unseen);
        return;
      case 'error':
        error.value = event.message;
        return;
      default: {
        const unknown: never = event;
        return unknown;
      }
    }
  }

  watching = invoke<void>('pacer_watch', { onEvent: new Channel<WatchEvent>(receive) }).catch((e: unknown) => {
    error.value = e instanceof Error ? e.message : String(e);
  });

  const tree = computed(() => buildTree(entities.value));
  const sessions = computed(() => new Map(tree.value.flatMap((w) => w.projects.flatMap((p) => p.worktrees.flatMap((t) => t.sessions.map((s) => [s.key, s] as const))))));
  return { connection, error, tree, sessions };
}

export function usePacer(): PacerStore | null {
  if (!('__TAURI_INTERNALS__' in window)) return null;
  store ??= createStore();
  return store;
}

export async function attachSession(session: SessionRef, cols: number, rows: number, onEvent: (event: SessionEvent) => void): Promise<void> {
  await watching;
  await invoke<void>('pacer_attach', { session, cols, rows, onEvent: new Channel<SessionEvent>(onEvent) });
}

export function detachSession(session: SessionRef): Promise<void> {
  return invoke<void>('pacer_detach', { session });
}

export function sendInput(session: SessionRef, data: string): Promise<void> {
  return invoke<void>('pacer_input', { session, data });
}

export function resizeSession(session: SessionRef, cols: number, rows: number): Promise<void> {
  return invoke<void>('pacer_resize', { session, cols, rows });
}

export function markSeen(agent: string): Promise<void> {
  return invoke<void>('pacer_mark_seen', { agent });
}
