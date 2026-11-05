import { computed, type ComputedRef } from 'vue';
import { usePacer, worktreesOfProject } from '../pacer';

export type WorktreeOption = { id: string; label: string };

export function useWorktreeOptions(project: () => string | undefined, except: () => string | undefined = () => undefined): ComputedRef<WorktreeOption[]> {
  const pacer = usePacer();
  return computed(() => {
    const projectId = project();
    if (!pacer || !projectId) return [];
    return worktreesOfProject(pacer.entities.value, projectId)
      .filter((worktree) => worktree.id !== except())
      .map((worktree) => ({ id: worktree.id, label: worktree.is_main ? `${worktree.branch} · основной` : worktree.branch }));
  });
}
