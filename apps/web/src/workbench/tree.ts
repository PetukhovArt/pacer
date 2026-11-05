import type { Topic } from '@core/client-sdk';
import type { Task } from '@veyra/protocol';
import { Archive, Eye, Folder, FolderGit2, FolderOpen, GitBranch, GitPullRequest, Hash, House, Inbox, Layers, ListTodo, SquareTerminal, UserRound } from 'lucide-vue-next';
import { worktreesOfProject, type Entities, type SessionNode, type WorktreeNode } from '../agents/pacer';
import { rollupOf, type Rollup, type WorkspaceNode } from '../agents/proto/nav';
import type { ConnectorView, InboxItem, JiraIssueSummary, PipelineStatus } from '../connectors/api';
import { numberLabel } from '../forge/format';
import { itemId, pipelineRollup } from '../forge/inbox';
import { t } from '../i18n';
import type { TreeNode } from '../tree/types';
import type { RepoRef } from './clones';
import { issuePanelId, taskPanelId } from './dock';
import { connectorBase, type Remote } from './remotes';

export type TreeMode = 'project' | 'type';

type NodeData = {
  rollup?: Rollup;
  pipeline?: PipelineStatus;
  workspaceId?: string;
  projectId?: string;
  worktree?: WorktreeNode;
  worktreeMr?: InboxItem;
  session?: SessionNode;
  item?: InboxItem;
  issue?: JiraIssueSummary;
  task?: Task;
  topic?: Topic;
  repo?: RepoRef;
};

export type WorkNode = TreeNode<NodeData>;

export type Sources = {
  pacer: boolean;
  relay: boolean;
  workspaces: readonly WorkspaceNode[];
  entities: Entities | undefined;
  remotes: ReadonlyMap<string, Remote | null>;
  connectors: readonly ConnectorView[];
  mrs: readonly InboxItem[];
  issues: readonly JiraIssueSummary[];
  tasks: readonly Task[];
  topics: readonly Topic[];
  jiraProjects: Readonly<Record<string, string>>;
};

type ProjectNode = WorkspaceNode['projects'][number];
type Work = JiraIssueSummary | Task;
type Links = {
  mrs: Map<string | null, InboxItem[]>;
  work: Map<string | null, Work[]>;
  worktreeMr: Map<string, InboxItem>;
  repoOf: Map<string, RepoRef>;
};

const JIRA_KEY = /[A-Z][A-Z0-9]+-\d+/g;

const push = <K, V>(map: Map<K, V[]>, key: K, value: V) => map.set(key, [...(map.get(key) ?? []), value]);

function link(sources: Sources): Links {
  const entities = sources.entities;
  const projects = entities ? [...entities.projects.values()] : [];
  const bases = new Map(sources.connectors.map((connector) => [connector.id, connectorBase(connector.base_url)]));
  const links: Links = { mrs: new Map(), work: new Map(), worktreeMr: new Map(), repoOf: new Map() };

  for (const item of [...sources.mrs].sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at))) {
    const base = bases.get(item.connector);
    const path = [base?.path, item.project].filter(Boolean).join('/');
    const full = path.toLowerCase();
    const host = base?.host ?? item.connector;
    links.repoOf.set(itemId(item), { key: `${host}/${full}`, host, path });
    const project = base
      ? projects.find((candidate) => {
          const remote = sources.remotes.get(candidate.repo_path);
          return remote?.host === base.host && remote.path.toLowerCase() === full;
        })
      : undefined;
    push(links.mrs, project?.id ?? null, item);
    if (!project || !entities) continue;
    for (const worktree of worktreesOfProject(entities, project.id)) if (worktree.branch === item.source_branch && !links.worktreeMr.has(worktree.id)) links.worktreeMr.set(worktree.id, item);
  }

  const byBranchKey = new Map<string, string>();
  if (entities)
    for (const project of projects)
      for (const worktree of worktreesOfProject(entities, project.id))
        for (const key of worktree.branch.toUpperCase().match(JIRA_KEY) ?? []) if (!byBranchKey.has(key)) byBranchKey.set(key, project.id);
  const byPrefix = new Map<string, string>();
  for (const [prefix, name] of Object.entries(sources.jiraProjects)) {
    const wanted = name.trim().toLowerCase();
    const project = projects.find((candidate) => candidate.name.toLowerCase() === wanted || candidate.id === name);
    if (project) byPrefix.set(prefix.trim().toUpperCase(), project.id);
  }
  const projectOfKey = (key: string) => byBranchKey.get(key.toUpperCase()) ?? byPrefix.get(key.slice(0, key.lastIndexOf('-')).toUpperCase()) ?? null;

  const issueKeys = new Set(sources.issues.map((issue) => issue.key));
  for (const issue of sources.issues) push(links.work, projectOfKey(issue.key), issue);
  for (const task of sources.tasks) if (!issueKeys.has(task.externalRef)) push(links.work, projectOfKey(task.externalRef), task);
  return links;
}

function matcher(query: string): (text: string) => boolean {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return (text) => {
    const haystack = text.toLowerCase();
    return words.every((word) => haystack.includes(word));
  };
}

function countLeaves(nodes: readonly WorkNode[]): number {
  return nodes.reduce((sum, node) => sum + (node.children ? countLeaves(node.children) : node.inert ? 0 : 1), 0);
}

function group(id: string, kind: string, label: string, children: WorkNode[], extra: Partial<WorkNode> = {}): WorkNode {
  const count = countLeaves(children);
  return { id, kind, label, children, count: count || undefined, countTitle: t('workbench.countTitle', { count }), ...extra };
}

const sessionsCount = (rollup: Rollup) => (rollup.total ? { count: `${rollup.running}/${rollup.total}`, countTitle: t('agents.layouts.runningTotal') } : { count: undefined });

const sessionLeaf = (session: SessionNode): WorkNode => ({ id: session.key, kind: 'session', label: session.name, custom: true, data: { session } });
const archivedLeaf = (session: SessionNode): WorkNode => ({ id: `arch:${session.key}`, kind: 'archived', label: session.name, custom: true, data: { session } });
const mrLeaf = (item: InboxItem): WorkNode => ({ id: itemId(item), kind: 'mr', label: `${numberLabel(item)} ${item.title}`, title: `${item.project} ${numberLabel(item)}\n${item.title}`, data: { item, pipeline: item.pipeline } });
const workLeaf = (work: Work): WorkNode =>
  'key' in work
    ? { id: issuePanelId(work.connector, work.key), kind: 'issue', label: `${work.key} ${work.summary}`, title: `${work.key} ${work.summary}\n${work.status}`, data: { issue: work } }
    : { id: taskPanelId(work.id), kind: 'task', label: `${work.externalRef} ${work.title}`, title: `${work.externalRef} ${work.title}\n${work.status}`, data: { task: work } };

const mrText = (item: InboxItem) => `${item.project} ${numberLabel(item)} ${item.number} ${item.title} ${item.author.name} ${item.author.username} ${item.source_branch} ${item.target_branch}`;
const workText = (work: Work) => ('key' in work ? `${work.key} ${work.summary} ${work.status}` : `${work.externalRef} ${work.title} ${work.status}`);

function worktreeNodes(prefix: string, project: ProjectNode, links: Links, match: (text: string) => boolean, filtering: boolean): WorkNode[] {
  return project.worktrees.flatMap((worktree) => {
    const sessions = worktree.sessions.filter((session) => match(`${session.name} ${worktree.branch} ${project.name}`));
    const archived = worktree.archived.filter((session) => match(`${session.name} ${worktree.branch} ${project.name}`));
    if (filtering && !sessions.length && !archived.length) return [];
    const children = sessions.map(sessionLeaf);
    if (archived.length)
      children.push({
        id: `${prefix}ar:${worktree.id}`,
        kind: 'archive',
        label: t('agents.layouts.archive'),
        icon: Archive,
        children: archived.map(archivedLeaf),
        count: archived.length,
        countTitle: t('agents.layouts.archivedCount'),
        defaultExpanded: false,
      });
    const rollup = rollupOf(worktree.sessions);
    return [
      {
        id: `${prefix}wt:${worktree.id}`,
        kind: 'worktree',
        label: worktree.branch,
        title: worktree.path,
        icon: worktree.main ? House : GitBranch,
        children,
        ...sessionsCount(rollup),
        data: { worktree, rollup, worktreeMr: links.worktreeMr.get(worktree.id) },
      },
    ];
  });
}

function mrSections(prefix: string, items: readonly InboxItem[]): WorkNode[] {
  const review = items.filter((item) => item.reasons.includes('reviewer'));
  const mine = items.filter((item) => !item.reasons.includes('reviewer'));
  const sections = [
    { key: 'review', label: t('workbench.awaitingReview'), icon: Eye, list: review },
    { key: 'mine', label: t('workbench.mine'), icon: UserRound, list: mine },
  ];
  return sections.flatMap((section) =>
    section.list.length ? [group(`${prefix}|${section.key}`, 'section', section.label, section.list.map(mrLeaf), { icon: section.icon, data: { pipeline: pipelineRollup(section.list) } })] : [],
  );
}

function mrsGroup(id: string, items: readonly InboxItem[]): WorkNode {
  return group(id, 'mrs', t('workbench.mergeRequests'), mrSections(id, items), { icon: GitPullRequest, data: { pipeline: pipelineRollup(items) } });
}

function repoNodes(prefix: string, items: readonly InboxItem[], repoOf: ReadonlyMap<string, RepoRef>, inner: (id: string, items: readonly InboxItem[]) => WorkNode[]): WorkNode[] {
  const byRepo = new Map<string, { repo: RepoRef; items: InboxItem[] }>();
  for (const item of items) {
    const repo = repoOf.get(itemId(item));
    if (!repo) continue;
    const entry = byRepo.get(repo.key) ?? { repo, items: [] };
    entry.items.push(item);
    byRepo.set(repo.key, entry);
  }
  const name = (repo: RepoRef) => repo.path.slice(repo.path.lastIndexOf('/') + 1);
  return [...byRepo.values()]
    .sort((a, b) => name(a.repo).localeCompare(name(b.repo)) || a.repo.key.localeCompare(b.repo.key))
    .map(({ repo, items: list }) => {
      const id = `${prefix}|repo:${repo.key}`;
      return group(id, 'repo', name(repo), inner(id, list), { icon: FolderGit2, detail: repo.path.slice(0, Math.max(0, repo.path.lastIndexOf('/'))) || undefined, title: `${repo.host}/${repo.path}`, data: { repo, pipeline: pipelineRollup(list) } });
    });
}

function workGroup(id: string, items: readonly Work[]): WorkNode {
  return group(id, 'tasks', t('workbench.tasks'), items.map(workLeaf), { icon: ListTodo });
}

function sessionsOf(project: ProjectNode): SessionNode[] {
  return project.worktrees.flatMap((worktree) => worktree.sessions);
}

export function buildTree(sources: Sources, mode: TreeMode, query: string): WorkNode[] {
  const links = link(sources);
  const match = matcher(query);
  const filtering = query.trim().length > 0;
  const mrsOf = (key: string | null) => (links.mrs.get(key) ?? []).filter((item) => match(mrText(item)));
  const workOf = (key: string | null) => (links.work.get(key) ?? []).filter((work) => match(workText(work)));
  const topics = sources.topics.filter((topic) => match(topic.name));
  const topicsGroup =
    topics.length || (sources.relay && !filtering)
      ? [group('topics', 'topics', t('workbench.topics'), topics.map((topic): WorkNode => ({ id: `topic:${topic.id}`, kind: 'topic', label: topic.name, icon: Hash, data: { topic } })), { icon: Hash })]
      : [];
  const projects = sources.workspaces.flatMap((workspace) => workspace.projects.map((project) => ({ workspace, project })));
  const multiWorkspace = sources.workspaces.length > 1;
  const projectLabel = (workspace: WorkspaceNode, project: ProjectNode) => (multiWorkspace ? `${workspace.name} / ${project.name}` : project.name);

  if (mode === 'type') {
    const sessionChildren = projects.flatMap(({ workspace, project }) => {
      const worktrees = worktreeNodes('s|', project, links, match, filtering);
      if (filtering && !worktrees.length) return [];
      const rollup = rollupOf(sessionsOf(project));
      return [{ id: `s|pr:${project.id}`, kind: 'project', label: projectLabel(workspace, project), icon: Folder, iconOpen: FolderOpen, children: worktrees, ...sessionsCount(rollup), data: { projectId: project.id, rollup } }];
    });
    const allSessions = projects.flatMap(({ project }) => sessionsOf(project));
    const sessionsRollup = rollupOf(allSessions);
    const mrChildren = [
      ...projects.flatMap(({ workspace, project }) => {
        const items = mrsOf(project.id);
        return items.length ? [group(`m|pr:${project.id}`, 'project', projectLabel(workspace, project), mrSections(`m|pr:${project.id}`, items), { icon: Folder, iconOpen: FolderOpen, data: { pipeline: pipelineRollup(items) } })] : [];
      }),
      ...(mrsOf(null).length ? [group('m|orphans', 'orphans', t('workbench.notInProject'), repoNodes('m|orphans', mrsOf(null), links.repoOf, mrSections), { icon: Inbox, data: { pipeline: pipelineRollup(mrsOf(null)) } })] : []),
    ];
    const workChildren = [
      ...projects.flatMap(({ workspace, project }) => {
        const items = workOf(project.id);
        return items.length ? [group(`w|pr:${project.id}`, 'project', projectLabel(workspace, project), items.map(workLeaf), { icon: Folder, iconOpen: FolderOpen })] : [];
      }),
      ...(workOf(null).length ? [group('w|orphans', 'orphans', t('workbench.notInProject'), workOf(null).map(workLeaf), { icon: Inbox })] : []),
    ];
    const allMrs = [...links.mrs.values()].flat();
    return [
      ...(sources.pacer && (!filtering || sessionChildren.length)
        ? [{ id: 'sessions', kind: 'sessions', label: t('workbench.sessions'), icon: SquareTerminal, children: sessionChildren, ...sessionsCount(sessionsRollup), data: { rollup: sessionsRollup } }]
        : []),
      ...(mrChildren.length ? [group('mrs', 'mrs', t('workbench.mergeRequests'), mrChildren, { icon: GitPullRequest, data: { pipeline: pipelineRollup(allMrs) } })] : []),
      ...(workChildren.length ? [group('tasks', 'tasks', t('workbench.tasks'), workChildren, { icon: ListTodo })] : []),
      ...topicsGroup,
    ];
  }

  const workspaceNodes = sources.workspaces.flatMap((workspace): WorkNode[] => {
    const children = workspace.projects.flatMap((project): WorkNode[] => {
      const worktrees = worktreeNodes('', project, links, match, filtering);
      const rollup = rollupOf(sessionsOf(project));
      const items = mrsOf(project.id);
      const work = workOf(project.id);
      const sections: WorkNode[] = [];
      if (!filtering || worktrees.length) sections.push({ id: `pr:${project.id}|sessions`, kind: 'sessions', label: t('workbench.sessions'), icon: SquareTerminal, children: worktrees, ...sessionsCount(rollup), data: { rollup } });
      if (items.length) sections.push(mrsGroup(`pr:${project.id}|mrs`, items));
      if (work.length) sections.push(workGroup(`pr:${project.id}|tasks`, work));
      if (filtering && !sections.length) return [];
      return [{ id: `pr:${project.id}`, kind: 'project', label: project.name, icon: Folder, iconOpen: FolderOpen, children: sections, data: { projectId: project.id, rollup, pipeline: pipelineRollup(items) } }];
    });
    if (filtering && !children.length) return [];
    if (!filtering) children.push({ id: `add-project:${workspace.id}`, kind: 'add-project', label: '', custom: true, inert: true, data: { workspaceId: workspace.id } });
    const rollup = rollupOf(workspace.projects.flatMap(sessionsOf));
    return [{ id: `ws:${workspace.id}`, kind: 'workspace', label: workspace.name, icon: Layers, children, ...sessionsCount(rollup), data: { workspaceId: workspace.id, rollup } }];
  });

  const orphanMrs = mrsOf(null);
  const orphanWork = workOf(null);
  const orphans = [
    ...repoNodes('orphans', orphanMrs, links.repoOf, (id, list) => [mrsGroup(`${id}|mrs`, list)]),
    ...(orphanWork.length ? [workGroup('orphans|tasks', orphanWork)] : []),
  ];
  return [
    ...workspaceNodes,
    ...(orphans.length ? [group('orphans', 'orphans', t('workbench.notInProject'), orphans, { icon: Inbox, data: { pipeline: pipelineRollup(orphanMrs) } })] : []),
    ...topicsGroup,
    ...(sources.pacer && !filtering ? [{ id: 'add-workspace', kind: 'add-workspace', label: '', custom: true, inert: true }] : []),
  ];
}

export type QuickItem = { key: string; node: WorkNode; crumbs: string[]; search: string };

const CRUMB_KINDS = new Set(['workspace', 'project', 'worktree', 'orphans', 'repo']);
const QUICK_KINDS = new Set(['session', 'mr', 'issue', 'task', 'topic']);

export function quickItems(nodes: readonly WorkNode[]): QuickItem[] {
  const out: QuickItem[] = [];
  const walk = (list: readonly WorkNode[], crumbs: string[]) => {
    for (const node of list) {
      if (node.children) walk(node.children, CRUMB_KINDS.has(node.kind) ? [...crumbs, node.label] : crumbs);
      else if (QUICK_KINDS.has(node.kind)) {
        const extra = node.data?.item ? ` ${mrText(node.data.item)}` : '';
        out.push({ key: node.id, node, crumbs, search: `${crumbs.join(' › ')} › ${node.label}${extra}`.toLowerCase() });
      }
    }
  };
  walk(nodes, []);
  const rank = (item: QuickItem) => (item.node.kind === 'session' ? 0 : item.node.kind === 'mr' ? 1 : 2);
  return out.sort((a, b) => rank(a) - rank(b));
}
