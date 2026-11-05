import { fsListDirs } from '../files/api';
import { inspectClone } from './remotes';

export type RepoRef = { key: string; host: string; path: string };
export type KnownProject = { repoPath: string; workspaceId: string };
export type Clone = { path: string; name: string; main: boolean; workspaceId: string };

const BATCH = 16;

const normalize = (path: string) => path.replace(/^\\\\\?\\/, '').replace(/\\/g, '/').replace(/\/+$/, '');
const parentOf = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf('/')));

export async function findClones(repo: RepoRef, projects: readonly KnownProject[]): Promise<Clone[]> {
  const known = new Set(projects.map((project) => normalize(project.repoPath).toLowerCase()));
  const parents = new Map<string, string>();
  for (const project of projects) {
    const parent = parentOf(normalize(project.repoPath));
    if (parent && !parents.has(parent)) parents.set(parent, project.workspaceId);
  }

  const candidates: { path: string; name: string; workspaceId: string }[] = [];
  for (const [parent, workspaceId] of parents) {
    let names: string[] = [];
    try {
      names = await fsListDirs(parent);
    } catch {
      continue;
    }
    for (const name of names) {
      const path = `${parent}/${name}`;
      if (!known.has(path.toLowerCase())) candidates.push({ path, name, workspaceId });
    }
  }

  const wanted = repo.path.toLowerCase();
  const found: Clone[] = [];
  for (let from = 0; from < candidates.length; from += BATCH) {
    const batch = candidates.slice(from, from + BATCH);
    const inspected = await Promise.all(batch.map((candidate) => inspectClone(candidate.path)));
    inspected.forEach(({ remote, main }, index) => {
      const candidate = batch[index];
      if (candidate && remote && remote.host === repo.host && remote.path.toLowerCase() === wanted) found.push({ ...candidate, main });
    });
  }
  return found.sort((a, b) => Number(b.main) - Number(a.main) || a.name.localeCompare(b.name));
}
