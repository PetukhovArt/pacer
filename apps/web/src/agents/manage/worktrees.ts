import { computed, type ComputedRef } from 'vue';
import { t } from '../../i18n';
import { useBerth, worktreesOfProject } from '../berth';

export type WorktreeOption = { id: string; label: string };

export function useWorktreeOptions(project: () => string | undefined, except: () => string | undefined = () => undefined): ComputedRef<WorktreeOption[]> {
  const berth = useBerth();
  return computed(() => {
    const projectId = project();
    if (!berth || !projectId) return [];
    return worktreesOfProject(berth.entities.value, projectId)
      .filter((worktree) => worktree.id !== except())
      .map((worktree) => ({ id: worktree.id, label: worktree.is_main ? t('agents.worktree.main', { branch: worktree.branch }) : worktree.branch }));
  });
}
