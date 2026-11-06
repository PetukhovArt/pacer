import { Archive, Eye, EyeOff, Folder, FolderGit2, FolderOpen, FolderSearch, GitBranch, History, House, Inbox, Layers, UserRound } from 'lucide-vue-next';
import { externalDot, externalKey, placeExternal, splitEarlier, type ExternalSession } from '../agents/external';
import { worktreesOfProject, type Entities, type SessionNode, type WorktreeNode } from '../agents/berth';
import { rollupOf, type Rollup, type WorkspaceNode } from '../agents/proto/nav';
import type { ConnectorView, InboxItem, JiraIssueSummary, PipelineStatus } from '../connectors/api';
import { numberLabel } from '../forge/format';
import { itemId, pipelineRollup } from '../forge/inbox';
import { t } from '../i18n';
import type { TreeNode } from '../tree/types';
import type { RepoRef } from './clones';
import { issuePanelId } from './dock';
import { connectorBase, type Remote } from './remotes';

export type TreeTab = 'sessions' | 'mrs' | 'reviews' | 'tasks' | 'all';

type NodeData = {
  rollup?: Rollup;
  pipeline?: PipelineStatus;
  workspaceId?: string;
  projectId?: string;
  worktree?: WorktreeNode;
  worktreeMr?: InboxItem;
  session?: SessionNode;
  external?: ExternalSession;
  linked?: ExternalSession;
  item?: InboxItem;
  issue?: JiraIssueSummary;
  repo?: RepoRef;
  review?: ReviewMeta;
  notes?: ReviewNoteView[];
  reviewState?: ReviewState;
};

export type WorkNode = TreeNode<NodeData>;

export type ReviewMeta = { refs: string[]; approved: boolean };
export type ReviewNoteView = { refs: string[]; text: string };
export type ReviewState = { loading: boolean; error: string | null };
export type ReviewSource = {
  rows: ReadonlyMap<string, ReviewMeta>;
  hosts: ReadonlySet<string>;
  byProject: ReadonlyMap<string, { notes: ReviewNoteView[]; state: ReviewState }>;
};

type ReviewKind = 'queue' | 'plain' | 'also';

export type Sources = {
  berth: boolean;
  workspaces: readonly WorkspaceNode[];
  entities: Entities | undefined;
  external: readonly ExternalSession[];
  linked: ReadonlyMap<string, ExternalSession>;
  remotes: ReadonlyMap<string, Remote | null>;
  connectors: readonly ConnectorView[];
  mrs: readonly InboxItem[];
  issues: readonly JiraIssueSummary[];
  jiraProjects: Readonly<Record<string, string>>;
  review: ReviewSource;
};

type ProjectNode = WorkspaceNode['projects'][number];
type Work = JiraIssueSummary;
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

  for (const issue of sources.issues) push(links.work, projectOfKey(issue.key), issue);
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

const sessionLeaf =
  (linked: ReadonlyMap<string, ExternalSession>) =>
  (session: SessionNode): WorkNode => {
    const link = linked.get(session.key);
    return { id: session.key, kind: 'session', label: link?.title ?? session.name, custom: true, data: link ? { session, linked: link } : { session } };
  };
const archivedLeaf = (session: SessionNode): WorkNode => ({ id: `arch:${session.key}`, kind: 'archived', label: session.name, custom: true, data: { session } });
const mrLeaf = (item: InboxItem, review?: ReviewMeta): WorkNode => ({
  id: itemId(item),
  kind: 'mr',
  label: `${numberLabel(item)} ${item.title}`,
  title: `${item.project} ${numberLabel(item)}\n${item.title}${review?.refs.length ? `\n${review.refs.join(', ')}` : ''}`,
  data: { item, pipeline: item.pipeline, review },
});
const workLeaf = (work: Work): WorkNode => ({ id: issuePanelId(work.connector, work.key), kind: 'issue', label: `${work.key} ${work.summary}`, title: `${work.key} ${work.summary}\n${work.status}`, data: { issue: work } });

const mrText = (item: InboxItem) => `${item.project} ${numberLabel(item)} ${item.number} ${item.title} ${item.author.name} ${item.author.username} ${item.source_branch} ${item.target_branch}`;
const workText = (work: Work) => `${work.key} ${work.summary} ${work.status}`;

type Placed = {
  now: number;
  entities: Entities | undefined;
  linked: ReadonlyMap<string, ExternalSession>;
  byWorktree: Map<string, ExternalSession[]>;
  byProject: Map<string, ExternalSession[]>;
  ofProject: Map<string, ExternalSession[]>;
  loose: Map<string, ExternalSession[]>;
};

function place(sources: Sources): Placed {
  const placed: Placed = { now: Date.now(), entities: sources.entities, linked: sources.linked, byWorktree: new Map(), byProject: new Map(), ofProject: new Map(), loose: new Map() };
  for (const session of sources.external) {
    const where = placeExternal(session.cwd, sources.entities);
    if (where.kind === 'loose') {
      push(placed.loose, where.folder, session);
      continue;
    }
    push(placed.ofProject, where.projectId, session);
    if (where.kind === 'worktree') push(placed.byWorktree, where.worktreeId, session);
    else push(placed.byProject, where.projectId, session);
  }
  return placed;
}

const externalText = (session: ExternalSession) => `${session.title} ${session.branch ?? ''} ${session.cwd} ${session.harness}`;
const externalLeaf = (session: ExternalSession): WorkNode => ({ id: externalKey(session), kind: 'external', label: session.title, title: session.cwd, custom: true, data: { external: session } });
const liveRollup = (sessions: readonly SessionNode[], external: readonly ExternalSession[], linked: ReadonlyMap<string, ExternalSession> = new Map()) =>
  rollupOf([
    ...sessions.map((session) => {
      const link = linked.get(session.key);
      return link ? { status: externalDot(link), unseen: false } : session;
    }),
    ...external.filter((session) => session.running).map((session) => ({ status: externalDot(session), unseen: false })),
  ]);

function externalChildren(prefix: string, sessions: readonly ExternalSession[], now: number): WorkNode[] {
  const { recent, earlier } = splitEarlier(sessions, now);
  const children = recent.map(externalLeaf);
  if (earlier.length)
    children.push({
      id: `${prefix}|earlier`,
      kind: 'earlier',
      label: t('workbench.external.earlier'),
      icon: History,
      children: earlier.map(externalLeaf),
      count: earlier.length,
      countTitle: t('workbench.countTitle', { count: earlier.length }),
      defaultExpanded: false,
    });
  return children;
}

function projectWorktrees(project: ProjectNode, placed: Placed): WorktreeNode[] {
  if (!placed.entities) return project.worktrees;
  const known = new Map(project.worktrees.map((worktree) => [worktree.id, worktree]));
  return worktreesOfProject(placed.entities, project.id).flatMap((worktree) => {
    const node = known.get(worktree.id);
    if (node) return [node];
    return placed.byWorktree.has(worktree.id) ? [{ id: worktree.id, path: worktree.path, branch: worktree.branch, main: worktree.is_main, sessions: [], archived: [] }] : [];
  });
}

function otherFolders(prefix: string, project: ProjectNode, placed: Placed, match: (text: string) => boolean): WorkNode[] {
  const list = (placed.byProject.get(project.id) ?? []).filter((session) => match(`${externalText(session)} ${project.name}`));
  if (!list.length) return [];
  const id = `${prefix}of:${project.id}`;
  return [group(id, 'folders', t('workbench.external.otherFolders'), externalChildren(id, list, placed.now), { icon: FolderSearch, data: { rollup: liveRollup([], list) } })];
}

function looseFolders(prefix: string, placed: Placed, match: (text: string) => boolean): WorkNode[] {
  return [...placed.loose.entries()].flatMap(([folder, list]) => {
    const shown = list.filter((session) => match(externalText(session)));
    const first = shown[0];
    if (!first) return [];
    const cwd = first.cwd.replaceAll('\\', '/').replace(/\/+$/, '');
    const cut = cwd.lastIndexOf('/');
    const id = `${prefix}|dir:${folder}`;
    return [group(id, 'folder', cwd.slice(cut + 1) || cwd, externalChildren(id, shown, placed.now), { icon: Folder, iconOpen: FolderOpen, detail: cut > 0 ? cwd.slice(0, cut) : undefined, title: cwd, data: { rollup: liveRollup([], shown) } })];
  });
}

function worktreeNodes(prefix: string, project: ProjectNode, links: Links, placed: Placed, match: (text: string) => boolean, filtering: boolean): WorkNode[] {
  return projectWorktrees(project, placed).flatMap((worktree) => {
    const linkedText = (session: SessionNode) => {
      const link = placed.linked.get(session.key);
      return link ? externalText(link) : '';
    };
    const sessions = worktree.sessions.filter((session) => match(`${session.name} ${linkedText(session)} ${worktree.branch} ${project.name}`));
    const archived = worktree.archived.filter((session) => match(`${session.name} ${worktree.branch} ${project.name}`));
    const placedHere = placed.byWorktree.get(worktree.id) ?? [];
    const external = placedHere.filter((session) => match(`${externalText(session)} ${worktree.branch} ${project.name}`));
    if (filtering && !sessions.length && !archived.length && !external.length) return [];
    const children = [...sessions.map(sessionLeaf(placed.linked)), ...externalChildren(`${prefix}ex:${worktree.id}`, external, placed.now)];
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
    const rollup = liveRollup(worktree.sessions, placedHere, placed.linked);
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

function mrSections(prefix: string, items: readonly InboxItem[], kinds: ReadonlyMap<string, ReviewKind>, reviews: ReadonlyMap<string, ReviewMeta>): WorkNode[] {
  const review = items.filter((item) => kinds.has(itemId(item)));
  const mine = items.filter((item) => !kinds.has(itemId(item)));
  const sections = [
    { key: 'review', label: t('workbench.awaitingReview'), icon: Eye, list: review },
    { key: 'mine', label: t('workbench.mine'), icon: UserRound, list: mine },
  ];
  return sections.flatMap((section) =>
    section.list.length ? [group(`${prefix}|${section.key}`, 'section', section.label, section.list.map((item) => mrLeaf(item, reviews.get(itemId(item)))), { icon: section.icon, data: { pipeline: pipelineRollup(section.list) } })] : [],
  );
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

function sessionsOf(project: ProjectNode): SessionNode[] {
  return project.worktrees.flatMap((worktree) => worktree.sessions);
}

function reviewKinds(sources: Sources): Map<string, ReviewKind> {
  const hostOf = new Map(sources.connectors.map((connector) => [connector.id, connectorBase(connector.base_url)?.host ?? connector.id]));
  const kinds = new Map<string, ReviewKind>();
  for (const item of sources.mrs) {
    const id = itemId(item);
    if (sources.review.rows.has(id)) kinds.set(id, 'queue');
    else if (item.reasons.includes('reviewer')) {
      if (!sources.review.hosts.has(hostOf.get(item.connector) ?? item.connector)) kinds.set(id, 'plain');
      else if (item.reasons.length === 1) kinds.set(id, 'also');
    }
  }
  return kinds;
}

const needsAttention = (item: InboxItem) => item.pipeline === 'failed' || (item.unresolved_threads ?? 0) > 0;

export type TabCounts = { sessionsWaiting: number; sessionsRunning: number; mrs: number; mrsAttention: number; reviews: number; tasks: number };

export function tabCounts(sources: Sources): TabCounts {
  const rollup = liveRollup(
    sources.workspaces.flatMap((workspace) => workspace.projects.flatMap(sessionsOf)),
    sources.external,
    sources.linked,
  );
  const kinds = reviewKinds(sources);
  const mine = sources.mrs.filter((item) => !kinds.has(itemId(item)));
  const pending = sources.mrs.filter((item) => {
    const kind = kinds.get(itemId(item));
    return kind === 'plain' || (kind === 'queue' && !sources.review.rows.get(itemId(item))?.approved);
  });
  return {
    sessionsWaiting: rollup.feedback,
    sessionsRunning: rollup.running,
    mrs: mine.length,
    mrsAttention: mine.filter(needsAttention).length,
    reviews: pending.length,
    tasks: sources.issues.length,
  };
}

export function buildTree(sources: Sources, tab: TreeTab, query: string): WorkNode[] {
  const links = link(sources);
  const match = matcher(query);
  const filtering = query.trim().length > 0;
  const placed = place(sources);
  const editable = tab === 'sessions';
  const projectRollup = (project: ProjectNode) => liveRollup(sessionsOf(project), placed.ofProject.get(project.id) ?? [], placed.linked);
  const kinds = reviewKinds(sources);
  const wanted = (item: InboxItem) => (tab === 'reviews' ? kinds.has(itemId(item)) : tab === 'mrs' ? !kinds.has(itemId(item)) : true);
  const mrsOf = (key: string | null) => (links.mrs.get(key) ?? []).filter((item) => wanted(item) && match(mrText(item)));
  const workOf = (key: string | null) => (links.work.get(key) ?? []).filter((work) => match(workText(work)));
  const projects = sources.workspaces.flatMap((workspace) => workspace.projects.map((project) => ({ workspace, project })));
  const projectLabel = (workspace: WorkspaceNode, project: ProjectNode) => (sources.workspaces.length > 1 ? `${workspace.name} / ${project.name}` : project.name);

  const sessionNodes = (): WorkNode[] => {
    if (!sources.berth && !sources.external.length) return [];
    const workspaceNodes = sources.workspaces.flatMap((workspace): WorkNode[] => {
      const children = workspace.projects.flatMap((project): WorkNode[] => {
        const worktrees = [...worktreeNodes('s|', project, links, placed, match, filtering), ...otherFolders('s|', project, placed, match)];
        if (filtering && !worktrees.length) return [];
        const rollup = projectRollup(project);
        return [{ id: `s|pr:${project.id}`, kind: 'project', label: project.name, icon: Folder, iconOpen: FolderOpen, children: worktrees, ...sessionsCount(rollup), data: { projectId: project.id, rollup } }];
      });
      if (filtering && !children.length) return [];
      if (editable && !filtering) children.push({ id: `add-project:${workspace.id}`, kind: 'add-project', label: '', custom: true, inert: true, data: { workspaceId: workspace.id } });
      const rollup = liveRollup(
        workspace.projects.flatMap(sessionsOf),
        workspace.projects.flatMap((project) => placed.ofProject.get(project.id) ?? []),
        placed.linked,
      );
      return [{ id: `ws:${workspace.id}`, kind: 'workspace', label: workspace.name, icon: Layers, children, ...sessionsCount(rollup), data: { workspaceId: workspace.id, rollup } }];
    });
    const loose = looseFolders('s|orphans', placed, match);
    return [...workspaceNodes, ...(loose.length ? [group('s|orphans', 'orphans', t('workbench.notInProject'), loose, { icon: Inbox, data: { rollup: liveRollup([], [...placed.loose.values()].flat()) } })] : [])];
  };

  const mrNodes = (): WorkNode[] => {
    const prefix = tab === 'reviews' ? 'r' : 'm';
    const reviewMeta = sources.review.rows;
    const leaf = (item: InboxItem) => mrLeaf(item, reviewMeta.get(itemId(item)));
    const reviewLeaves = (id: string, items: readonly InboxItem[]): WorkNode[] => {
      const also = items.filter((item) => kinds.get(itemId(item)) === 'also');
      const nodes = items.filter((item) => kinds.get(itemId(item)) !== 'also').map(leaf);
      if (also.length) nodes.push(group(`${id}|also`, 'also', t('workbench.review.also'), also.map(leaf), { icon: EyeOff, defaultExpanded: false, title: t('workbench.review.alsoTitle') }));
      return nodes;
    };
    const inner = (id: string, items: readonly InboxItem[]) => (tab === 'all' ? mrSections(id, items, kinds, reviewMeta) : tab === 'reviews' ? reviewLeaves(id, items) : items.map(leaf));
    const orphans = mrsOf(null);
    const adapterNodes = (id: string, projectId: string): WorkNode[] => {
      const review = tab === 'reviews' && !filtering ? sources.review.byProject.get(projectId) : undefined;
      if (!review) return [];
      const nodes: WorkNode[] = [];
      if (review.state.loading || review.state.error) nodes.push({ id: `${id}|state`, kind: 'review-state', label: review.state.error ?? t('workbench.review.loading'), custom: true, inert: true, data: { reviewState: review.state } });
      if (review.notes.length) nodes.push({ id: `${id}|notes`, kind: 'review-notes', label: t('workbench.review.notes'), custom: true, inert: true, data: { notes: review.notes } });
      return nodes;
    };
    return [
      ...projects.flatMap(({ workspace, project }) => {
        const items = mrsOf(project.id);
        const id = `${prefix}|pr:${project.id}`;
        const extra = adapterNodes(id, project.id);
        if (!items.length && !extra.length) return [];
        const children = [...extra, ...inner(id, items)];
        const shown = tab === 'reviews' ? items.filter((item) => kinds.get(itemId(item)) !== 'also').length : undefined;
        return [group(id, 'project', projectLabel(workspace, project), children, { icon: Folder, iconOpen: FolderOpen, data: { pipeline: pipelineRollup(items) }, ...(shown !== undefined ? { count: shown || undefined, countTitle: t('workbench.countTitle', { count: shown }) } : {}) })];
      }),
      ...(orphans.length ? [group(`${prefix}|orphans`, 'orphans', t('workbench.notInProject'), repoNodes(`${prefix}|orphans`, orphans, links.repoOf, inner), { icon: Inbox, data: { pipeline: pipelineRollup(orphans) } })] : []),
    ];
  };

  const taskNodes = (): WorkNode[] => {
    const orphans = workOf(null);
    return [
      ...projects.flatMap(({ workspace, project }) => {
        const items = workOf(project.id);
        return items.length ? [group(`w|pr:${project.id}`, 'project', projectLabel(workspace, project), items.map(workLeaf), { icon: Folder, iconOpen: FolderOpen })] : [];
      }),
      ...(orphans.length ? [group('w|orphans', 'orphans', t('workbench.notInProject'), orphans.map(workLeaf), { icon: Inbox })] : []),
    ];
  };

  if (tab === 'sessions') return [...sessionNodes(), ...(sources.berth && !filtering ? [{ id: 'add-workspace', kind: 'add-workspace', label: '', custom: true, inert: true }] : [])];
  if (tab === 'tasks') return taskNodes();
  if (tab === 'all') return [...sessionNodes(), ...mrNodes(), ...taskNodes()];
  return mrNodes();
}

export type QuickItem = { key: string; node: WorkNode; crumbs: string[]; search: string };

const CRUMB_KINDS = new Set(['workspace', 'project', 'worktree', 'orphans', 'repo', 'folder', 'folders']);
const QUICK_KINDS = new Set(['session', 'external', 'mr', 'issue']);
const QUICK_RANK: Readonly<Record<string, number>> = { session: 0, external: 1, mr: 2 };

export function quickItems(nodes: readonly WorkNode[]): QuickItem[] {
  const out: QuickItem[] = [];
  const walk = (list: readonly WorkNode[], crumbs: string[]) => {
    for (const node of list) {
      if (node.children) walk(node.children, CRUMB_KINDS.has(node.kind) ? [...crumbs, node.label] : crumbs);
      else if (QUICK_KINDS.has(node.kind)) {
        const extra = node.data?.item ? ` ${mrText(node.data.item)} ${node.data.review?.refs.join(' ') ?? ''}` : node.data?.external ? ` ${externalText(node.data.external)}` : node.data?.linked ? ` ${externalText(node.data.linked)}` : '';
        out.push({ key: node.id, node, crumbs, search: `${crumbs.join(' › ')} › ${node.label}${extra}`.toLowerCase() });
      }
    }
  };
  walk(nodes, []);
  const rank = (item: QuickItem) => QUICK_RANK[item.node.kind] ?? 3;
  return out.sort((a, b) => rank(a) - rank(b));
}
