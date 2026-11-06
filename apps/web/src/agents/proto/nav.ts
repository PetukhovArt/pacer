import type { SessionNode, SessionStatus, useBerth } from '../berth';

type Store = NonNullable<ReturnType<typeof useBerth>>;
export type WorkspaceNode = Store['tree']['value'][number];

export type SessionPath = {
  workspace: { id: string; name: string };
  project: { id: string; name: string };
  worktree: { id: string; branch: string };
  session: SessionNode;
};

export function sessionPaths(tree: readonly WorkspaceNode[]): Map<string, SessionPath> {
  const paths = new Map<string, SessionPath>();
  for (const workspace of tree)
    for (const project of workspace.projects)
      for (const worktree of project.worktrees)
        for (const session of worktree.sessions)
          paths.set(session.key, {
            workspace: { id: workspace.id, name: workspace.name },
            project: { id: project.id, name: project.name },
            worktree: { id: worktree.id, branch: worktree.branch },
            session,
          });
  return paths;
}

const URGENCY: Record<SessionStatus, number> = {
  needs_feedback: 7,
  running: 6,
  terminated: 5,
  finished: 4,
  fresh: 3,
  shell: 2,
  disconnected: 1,
  closed: 0,
};

export type Rollup = { status: SessionStatus | undefined; unseen: boolean; running: number; feedback: number; total: number };

export function rollupOf(sessions: readonly Pick<SessionNode, 'status' | 'unseen'>[]): Rollup {
  let status: SessionStatus | undefined;
  let unseen = false;
  let running = 0;
  let feedback = 0;
  for (const session of sessions) {
    if (status === undefined || URGENCY[session.status] > URGENCY[status]) status = session.status;
    if (session.unseen) unseen = true;
    if (session.status === 'running') running++;
    if (session.status === 'needs_feedback') feedback++;
  }
  return { status, unseen, running, feedback, total: sessions.length };
}
