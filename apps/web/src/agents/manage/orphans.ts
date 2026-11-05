import { acknowledged, call, PacerCallError, type PacerReply } from '../pacer';

export type OrphanedSession = Extract<PacerReply, { OrphanedSessions: unknown }>['OrphanedSessions']['sessions'][number];

export async function listOrphans(project: string): Promise<OrphanedSession[]> {
  const reply = await call({ ListOrphanedSessions: { project } });
  if (!('OrphanedSessions' in reply)) throw new PacerCallError(`pacer ответил ${Object.keys(reply).join('')} вместо списка разговоров`);
  return reply.OrphanedSessions.sessions;
}

export async function resumeOrphan(sessionId: string, worktree: string): Promise<string> {
  const created = await acknowledged({ ResumeOrphanedSession: { session_id: sessionId, worktree } });
  if (!created || !('Agent' in created)) throw new PacerCallError('pacer не вернул созданного агента');
  return created.Agent;
}

export async function moveAgent(agent: string, worktree: string): Promise<void> {
  await acknowledged({ MoveAgent: { id: agent, worktree } });
}
