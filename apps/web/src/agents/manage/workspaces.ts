import { acknowledged } from '../berth';

export type NewProject = { path: string; name: string | null; createMissing: boolean };

export async function addWorkspace(name: string): Promise<void> {
  await acknowledged({ AddWorkspace: { name } });
}

export async function removeWorkspace(id: string): Promise<void> {
  await acknowledged({ RemoveWorkspace: { id } });
}

export async function renameWorkspace(id: string, name: string): Promise<void> {
  await acknowledged({ RenameWorkspace: { id, name } });
}

export async function addProject(workspace: string, project: NewProject): Promise<void> {
  await acknowledged({ OpenWorkspace: { id: workspace } });
  await acknowledged({ AddProject: { path: project.path, name: project.name, create_missing: project.createMissing } });
}

export async function removeProject(id: string): Promise<void> {
  await acknowledged({ RemoveProject: { id } });
}

export async function renameProject(id: string, name: string): Promise<void> {
  await acknowledged({ RenameProject: { id, name } });
}
