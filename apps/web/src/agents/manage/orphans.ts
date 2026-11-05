import { t } from '../../i18n';
import { acknowledged, call, PacerCallError, type PacerReply } from '../pacer';

export type OrphanedSession = Extract<PacerReply, { OrphanedSessions: unknown }>['OrphanedSessions']['sessions'][number];

export async function listOrphans(project: string): Promise<OrphanedSession[]> {
  const reply = await call({ ListOrphanedSessions: { project } });
  if (!('OrphanedSessions' in reply)) throw new PacerCallError(t('agents.errors.expectedOrphans', { reply: Object.keys(reply).join('') }));
  return reply.OrphanedSessions.sessions;
}

export async function resumeOrphan(sessionId: string, worktree: string): Promise<string> {
  const created = await acknowledged({ ResumeOrphanedSession: { session_id: sessionId, worktree } });
  if (!created || !('Agent' in created)) throw new PacerCallError(t('agents.errors.noAgentCreated'));
  return created.Agent;
}

export async function moveAgent(agent: string, worktree: string): Promise<void> {
  await acknowledged({ MoveAgent: { id: agent, worktree } });
}
