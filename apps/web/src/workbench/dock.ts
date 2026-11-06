import { computed, onActivated, onBeforeUnmount, onDeactivated, shallowRef, watch } from 'vue';
import type { DockviewApi, DockviewReadyEvent } from 'dockview-vue';
import { externalKey, type ExternalSession } from '../agents/external';
import { markSeen, sessionKey, usePacer, type SessionNode, type SessionRef, type WorktreeNode } from '../agents/pacer';
import { FILE_PANEL_PREFIX, fileNameOf, filePanelId, isFilePanel, normalizePath } from '../agents/proto/markdownFiles';
import { sessionPaths } from '../agents/proto/nav';
import type { ChangeRequestRef } from '../connectors/api';
import { numberLabel } from '../forge/format';
import { itemId } from '../forge/inbox';
import { architecturePanelId, type ArchitectureParams } from '../forge/stratum';
import { t } from '../i18n';
import { useIsMobile } from '../mobile/useIsMobile';
import { navigate, openTopic, view } from '../nav';
import type { WorkNode } from './tree';

export const METRICS_PANEL = 'metrics';

type AddPanel = Parameters<DockviewApi['addPanel']>[0];
type PanelPosition = { referencePanel: string; direction: 'within' | 'right' | 'below' | 'left' };
type Carried = { id: string; component: string; title: string; params: Record<string, unknown> };

export const issuePanelId = (connector: string, key: string) => `jira:${connector}/${key}`;
export const taskPanelId = (id: string) => `task:${id}`;
const isSessionPanel = (id: string) => id.startsWith('agent:') || id.startsWith('terminal:');

function createDock() {
  const pacer = usePacer();
  const mobile = useIsMobile();
  const dock = shallowRef<DockviewApi | null>(null);
  const activeKey = shallowRef<string | null>(null);
  const activeTitle = shallowRef('');
  const visible = shallowRef(false);
  const creatingIn = shallowRef<string | null>(null);
  const openingKey = shallowRef<string | null>(null);
  const detail = shallowRef(false);
  const switching = shallowRef(false);
  const pending: ((api: DockviewApi) => void)[] = [];
  let carried: { panels: Carried[]; active: string | null } = { panels: [], active: null };

  const paths = computed(() => sessionPaths(pacer?.tree.value ?? []));
  const activePath = computed(() => (activeKey.value ? paths.value.get(activeKey.value) : undefined));
  const activeSession = computed(() => (activeKey.value ? pacer?.sessions.value.get(activeKey.value) : undefined));

  function findWorktree(id: string | null | undefined): WorktreeNode | null {
    if (!id) return null;
    for (const workspace of pacer?.tree.value ?? []) for (const project of workspace.projects) for (const worktree of project.worktrees) if (worktree.id === id) return worktree;
    return null;
  }

  const creatingWorktree = computed(() => findWorktree(creatingIn.value));
  const activeWorktree = computed(() => {
    const byPath = findWorktree(activePath.value?.worktree.id);
    const key = activeKey.value;
    if (byPath || !key || !isFilePanel(key)) return byPath;
    const file = key.slice(FILE_PANEL_PREFIX.length);
    let best: WorktreeNode | null = null;
    for (const workspace of pacer?.tree.value ?? [])
      for (const project of workspace.projects)
        for (const worktree of project.worktrees) {
          const root = normalizePath(worktree.path).replace(/\/+$/, '');
          if (file.startsWith(`${root}/`) && (!best || root.length > best.path.length)) best = worktree;
        }
    return best;
  });

  const activeUnseenAgent = computed(() => {
    const session = activeSession.value;
    return visible.value && session?.unseen && 'Agent' in session.ref ? session.ref.Agent : null;
  });

  watch(activeUnseenAgent, (agent) => {
    if (agent) void markSeen(agent);
  });

  watch(
    () => pacer?.sessions.value,
    (sessions) => {
      const api = dock.value;
      if (!api || !sessions || pacer?.connection.value.state !== 'connected') return;
      for (const panel of api.panels) if (isSessionPanel(panel.id) && !sessions.has(panel.id)) panel.api.close();
      const opening = openingKey.value ? sessions.get(openingKey.value) : undefined;
      if (!opening) return;
      openingKey.value = null;
      openSession(opening);
    },
  );

  function flush() {
    const api = dock.value;
    if (!api || !visible.value) return;
    for (const run of pending.splice(0)) run(api);
  }

  function withDock(run: (api: DockviewApi) => void, reveal = true) {
    if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
    if (reveal && mobile.value) detail.value = true;
    pending.push(run);
    flush();
  }

  function openPanel(panel: AddPanel) {
    withDock((api) => {
      const existing = api.getPanel(panel.id);
      if (existing) existing.api.setActive();
      else api.addPanel(panel);
    });
  }

  function sessionPosition(session: SessionNode, api: DockviewApi): PanelPosition | undefined {
    const worktree = paths.value.get(session.key)?.worktree.id;
    const sibling = api.panels.find((panel) => panel.id !== session.key && paths.value.get(panel.id)?.worktree.id === worktree);
    if (sibling) return { referencePanel: sibling.id, direction: 'within' };
    const lastSession = api.panels.filter((panel) => paths.value.has(panel.id)).at(-1);
    if (lastSession) return { referencePanel: lastSession.id, direction: 'right' };
    return undefined;
  }

  function addSessionPanel(session: SessionNode, api: DockviewApi) {
    const position = sessionPosition(session, api);
    api.addPanel({ id: session.key, component: 'session', title: session.name, params: { session: session.ref }, renderer: 'always', ...(position ? { position } : {}) });
  }

  function openSession(session: SessionNode) {
    withDock((api) => {
      const existing = api.getPanel(session.key);
      if (existing) {
        existing.api.setActive();
        return;
      }
      addSessionPanel(session, api);
    });
  }

  function openLinkedConversation(session: SessionNode, linked: ExternalSession) {
    withDock((api) => {
      const id = externalKey(linked);
      const existing = api.getPanel(id);
      if (existing) {
        existing.api.setActive();
        return;
      }
      if (!api.getPanel(session.key)) addSessionPanel(session, api);
      api.addPanel({ id, component: 'conversation', title: linked.title, params: { harness: linked.harness, id: linked.id }, position: { referencePanel: session.key, direction: 'within' } });
    });
  }

  function openFile(path: string, options: { hash?: string; worktreeId?: string; from?: string } = {}) {
    withDock((api) => {
      const id = filePanelId(path);
      const hash = options.hash ?? '';
      const existing = api.getPanel(id);
      if (existing) {
        existing.api.updateParameters({ path: normalizePath(path), hash });
        existing.api.setActive();
        return;
      }
      const anchor =
        options.from && api.getPanel(options.from)
          ? options.from
          : options.worktreeId
            ? api.panels.find((panel) => paths.value.get(panel.id)?.worktree.id === options.worktreeId)?.id
            : undefined;
      const position: PanelPosition | undefined = anchor ? { referencePanel: anchor, direction: 'within' } : undefined;
      api.addPanel({ id, component: 'markdown', title: fileNameOf(path), params: { path: normalizePath(path), hash }, renderer: 'always', ...(position ? { position } : {}) });
    });
  }

  function openReview(ref: ChangeRequestRef, title?: string) {
    openPanel({ id: itemId(ref), component: 'review', title: title ?? `${ref.project.split('/').pop() ?? ref.project} ${ref.number}`, params: { ...ref } });
  }

  function openArchitecture(params: ArchitectureParams, label: string) {
    openPanel({ id: architecturePanelId(params), component: 'architecture', title: t('forge.stratum.tab', { label }), renderer: 'always', params: { ...params } });
  }

  function openIssue(connector: string, key: string) {
    openPanel({ id: issuePanelId(connector, key), component: 'issue', title: key, params: { connector, key } });
  }

  function openTask(id: string, title: string) {
    openPanel({ id: taskPanelId(id), component: 'task', title, params: { id } });
  }

  function openExternal(session: ExternalSession) {
    openPanel({ id: externalKey(session), component: 'conversation', title: session.title, params: { harness: session.harness, id: session.id } });
  }

  function openLeaf(node: WorkNode) {
    const data = node.data;
    if (node.kind === 'session' && data?.session) openSession(data.session);
    else if (data?.external) openExternal(data.external);
    else if (data?.item) openReview(data.item, `${numberLabel(data.item)} ${data.item.title}`);
    else if (data?.issue) openIssue(data.issue.connector, data.issue.key);
    else if (data?.task) openTask(data.task.id, data.task.externalRef);
    else if (data?.topic) openTopic(data.topic.id);
  }

  function openMetrics() {
    openPanel({ id: METRICS_PANEL, component: 'metrics', title: t('agents.metrics.title') });
  }

  function openWhenListed(session: SessionRef) {
    const key = sessionKey(session);
    const node = pacer?.sessions.value.get(key);
    if (node) openSession(node);
    else openingKey.value = key;
  }

  function startCreate(worktreeId: string) {
    if (view.value.kind !== 'agents') void navigate({ kind: 'agents' });
    creatingIn.value = worktreeId;
  }

  function startCreateInMain() {
    const projects = (pacer?.tree.value ?? []).flatMap((workspace) => workspace.projects);
    const active = activeWorktree.value?.id;
    const project = projects.find((candidate) => candidate.worktrees.some((worktree) => worktree.id === active)) ?? projects[0];
    const target = project?.worktrees.find((worktree) => worktree.main) ?? project?.worktrees[0];
    if (target) startCreate(target.id);
  }

  function onCreated(session: SessionRef) {
    creatingIn.value = null;
    openWhenListed(session);
  }

  function restore(api: DockviewApi) {
    const sessions = pacer?.sessions.value;
    for (const panel of carried.panels) {
      if (isSessionPanel(panel.id) && !sessions?.has(panel.id)) continue;
      const always = panel.component === 'session' || panel.component === 'markdown' || panel.component === 'architecture';
      api.addPanel({ id: panel.id, component: panel.component, title: panel.title, params: panel.params, ...(always ? { renderer: 'always' as const } : {}) });
    }
    if (carried.active) api.getPanel(carried.active)?.api.setActive();
    carried = { panels: [], active: null };
  }

  function useHost() {
    let activeChange: { dispose(): void } | null = null;
    visible.value = true;

    function onReady({ api }: DockviewReadyEvent) {
      dock.value = api;
      activeChange = api.onDidActivePanelChange(({ panel }) => {
        activeKey.value = panel?.id ?? null;
        activeTitle.value = panel?.title ?? '';
      });
      const queued = pending.splice(0);
      restore(api);
      pending.push(...queued);
      flush();
    }

    onActivated(() => {
      visible.value = true;
      flush();
    });
    onDeactivated(() => {
      visible.value = false;
    });
    onBeforeUnmount(() => {
      const api = dock.value;
      if (api)
        carried = {
          panels: api.panels.map((panel) => ({ id: panel.id, component: panel.view.contentComponent, title: panel.title ?? panel.id, params: { ...panel.params } })),
          active: activeKey.value,
        };
      activeChange?.dispose();
      dock.value = null;
      visible.value = false;
    });

    return { onReady };
  }

  return {
    pacer,
    activeKey,
    activeTitle,
    activeSession,
    activeWorktree,
    paths,
    creatingWorktree,
    creatingIn,
    detail,
    switching,
    openSession,
    openLinkedConversation,
    openFile,
    openReview,
    openArchitecture,
    openIssue,
    openTask,
    openExternal,
    openLeaf,
    openMetrics,
    openWhenListed,
    startCreate,
    startCreateInMain,
    onCreated,
    useHost,
  };
}

let store: ReturnType<typeof createDock> | null = null;

export function useDock() {
  store ??= createDock();
  return store;
}

export function toggleQuickOpen(): void {
  const { switching } = useDock();
  switching.value = !switching.value;
}
