import { shallowRef } from 'vue';
import { acknowledged, createAgent, describeError, entitySessionRef, useBerth, worktreesOfProject, type AgentKind, type SessionRef } from '../agents/berth';
import { invoke } from '../bridge';
import type { AiView } from '../connectors/ai';
import { forgeChangeRequest, forgeSource, mrFetchSource, useConnectors, type ChangeRequestRef, type ChangeRequestSummary, type FetchedSource } from '../connectors/api';
import { t } from '../i18n';
import { useSettings } from '../settings/store';
import type { RepoRef } from '../workbench/clones';
import { useDock } from '../workbench/dock';
import { connectorBase, inspectClone, remotes, type Remote } from '../workbench/remotes';
import { projectOfRepo, repoRefOf } from '../workbench/tree';
import { numberLabel } from './format';
import { itemId } from './inbox';

export type MrItem = Pick<ChangeRequestSummary, 'connector' | 'project' | 'number' | 'title' | 'url' | 'source_branch' | 'target_branch' | 'author'>;

export type MrRun = {
  id: string;
  label: string;
  item: MrItem;
  refs: readonly string[];
  busy: boolean;
  message: string;
  error: string | null;
  warning: string | null;
  needsProject: RepoRef | null;
};

const DESCRIPTION_LIMIT = 1500;
const NAME_LIMIT = 60;
const TRACKER_KEY = /[A-Z][A-Z0-9]+-\d+/;
const KIND_LABEL: Record<AgentKind, string> = { claude: 'Claude', codex: 'Codex', cursor: 'Cursor' };
const KINDS = new Set<string>(['claude', 'codex', 'cursor']);

class Problem extends Error {}

export const mrRuns = shallowRef<ReadonlyMap<string, MrRun>>(new Map());

function put(run: MrRun) {
  mrRuns.value = new Map(mrRuns.value).set(run.id, run);
}

function dismissMrRun(id: string) {
  const next = new Map(mrRuns.value);
  next.delete(id);
  mrRuns.value = next;
}

function bounded(text: string): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  return flat.length > DESCRIPTION_LIMIT ? `${flat.slice(0, DESCRIPTION_LIMIT).trimEnd()}…` : flat;
}

type PromptFields = Record<'url' | 'title' | 'description' | 'branch' | 'target' | 'issue' | 'author', string>;

function composeMrPrompt(template: string, fields: PromptFields): string {
  return template
    .replace(/\{(url|title|description|branch|target|issue|author)\}/g, (_, key: keyof PromptFields) => fields[key])
    .replace(/\(\s*\)/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

function issueOf(item: MrItem, refs: readonly string[]): string {
  const known = refs.filter((ref) => !ref.startsWith('!'));
  if (known.length) return known.join(', ');
  return TRACKER_KEY.exec(item.source_branch.toUpperCase())?.[0] ?? TRACKER_KEY.exec(item.title)?.[0] ?? '';
}

function authorOf(item: MrItem): string {
  const { name, username } = item.author;
  return name && name !== username ? `${name} (@${username})` : `@${username}`;
}

function fetchProblem(fetched: FetchedSource, branch: string): string {
  const detail = (fetched.detail ?? '').split(/\r?\n/).filter(Boolean).at(-1) ?? '';
  switch (fetched.problem) {
    case 'branch_missing':
      return t('forge.openSession.errors.branchMissing', { branch, remote: fetched.remote });
    case 'no_clone_url':
      return t('forge.openSession.errors.noCloneUrl', { owner: fetched.remote });
    default:
      return t('forge.openSession.errors.fetchFailed', { branch, remote: fetched.remote, detail });
  }
}

async function ensureCli(kind: AgentKind) {
  if (kind === 'cursor') return;
  const view = await invoke<AiView>('ai_refresh', { kind }).catch(() => null);
  if (view?.status === 'not_installed') throw new Problem(t('forge.openSession.errors.cliMissing', { kind: KIND_LABEL[kind] }));
  if (view?.status === 'signed_out') throw new Problem(t('forge.openSession.errors.cliSignedOut', { kind: KIND_LABEL[kind] }));
}

async function knownRemotes(repos: readonly string[]): Promise<ReadonlyMap<string, Remote | null>> {
  const known = new Map(remotes.value);
  const missing = repos.filter((repo) => !known.has(repo));
  const found = await Promise.all(missing.map((repo) => inspectClone(repo)));
  missing.forEach((repo, index) => known.set(repo, found[index]?.remote ?? null));
  return known;
}

export function useOpenInSession() {
  const berth = useBerth();
  const dock = useDock();
  const { connectors } = useConnectors();
  const { values } = useSettings();

  async function open(item: MrItem, refs: readonly string[] = []) {
    const id = itemId(item);
    if (mrRuns.value.get(id)?.busy) return;
    let run: MrRun = { id, label: `${numberLabel(item)} ${item.title}`, item, refs, busy: true, message: t('forge.openSession.step.project'), error: null, warning: null, needsProject: null };
    const update = (patch: Partial<MrRun>) => {
      run = { ...run, ...patch };
      put(run);
    };
    put(run);
    try {
      if (!berth || berth.connection.value.state !== 'connected') throw new Problem(t('forge.openSession.errors.offline'));
      const connector = connectors.value.find((candidate) => candidate.id === item.connector);
      const repo = repoRefOf(item, connector ? connectorBase(connector.base_url) : null);
      const projects = [...berth.entities.value.projects.values()];
      const project = connector ? projectOfRepo(repo, projects, await knownRemotes(projects.map((candidate) => candidate.repo_path))) : undefined;
      if (!project) {
        update({ busy: false, needsProject: repo, message: t('forge.openSession.noProject', { repo: `${repo.host}/${repo.path}` }) });
        return;
      }

      update({ message: t('forge.openSession.step.source') });
      const ref: ChangeRequestRef = { connector: item.connector, project: item.project, number: item.number };
      const [request, source] = await Promise.all([forgeChangeRequest(ref), forgeSource(ref)]);
      if (source.gone) throw new Problem(t('forge.openSession.errors.sourceGone'));
      const fork = source.fork;
      if (fork && source.can_push === false) update({ warning: t('forge.openSession.forkReadOnly', { owner: fork.owner }) });
      const branch = source.branch || request.source_branch;
      const local = fork ? `${fork.owner}/${branch}` : branch;

      let worktree = worktreesOfProject(berth.entities.value, project.id).find((candidate) => candidate.branch === local)?.id;
      if (!worktree) {
        update({ message: t('forge.openSession.step.fetch', { branch, remote: fork?.owner ?? 'origin' }) });
        const fetched = await mrFetchSource(project.repo_path, branch, local, fork);
        if (fetched.problem || !fetched.base) throw new Problem(fetchProblem(fetched, branch));
        if (fetched.diverged) update({ warning: [run.warning, t('forge.openSession.diverged', { branch: local })].filter(Boolean).join(' ') });
        update({ message: t('forge.openSession.step.worktree', { branch: local }) });
        const created = await acknowledged({ CreateWorktree: { project: project.id, branch: local, base: fetched.base } });
        worktree = created && 'Worktree' in created ? created.Worktree : undefined;
        if (!worktree) throw new Problem(t('forge.openSession.errors.failed', { detail: 'CreateWorktree' }));
      }

      const here = worktree;
      const live = [...berth.entities.value.agents.values()]
        .filter((agent) => agent.worktree_id === here && !agent.archived)
        .sort((a, b) => Number(b.alive) - Number(a.alive) || b.status_changed_at - a.status_changed_at)[0];
      let session: SessionRef;
      if (live) session = { Agent: live.id };
      else {
        const chosen = values.value['sessions.default_kind'];
        const kind: AgentKind = KINDS.has(chosen) ? (chosen as AgentKind) : 'claude';
        update({ message: t('forge.openSession.step.agent', { kind: KIND_LABEL[kind] }) });
        await ensureCli(kind);
        const template = values.value['sessions.mrPrompt'].trim() || t('forge.openSession.prompt');
        const prompt = composeMrPrompt(template, {
          url: request.url,
          title: request.title,
          description: bounded(request.description),
          branch: local,
          target: request.target_branch,
          issue: issueOf(request, refs),
          author: authorOf(request),
        });
        const name = run.label.length > NAME_LIMIT ? `${run.label.slice(0, NAME_LIMIT - 1).trimEnd()}…` : run.label;
        const created = entitySessionRef(
          await createAgent({
            worktree: here,
            name,
            kind,
            model: values.value['sessions.default_model'].trim() || null,
            effort: values.value['sessions.default_effort'].trim() || null,
            autoTitle: false,
            startingPrompt: prompt,
          }),
        );
        if (!created) throw new Problem(t('forge.openSession.errors.failed', { detail: 'CreateAgent' }));
        session = created;
      }

      dock.openMrSession(session, ref, run.label);
      if (run.warning) update({ busy: false, message: '' });
      else dismissMrRun(id);
    } catch (e) {
      update({ busy: false, error: e instanceof Problem ? e.message : t('forge.openSession.errors.failed', { detail: describeError(e) }) });
    }
  }

  return { open, runs: mrRuns, dismiss: dismissMrRun };
}
