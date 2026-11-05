import { computed, onActivated, onBeforeUnmount, onDeactivated, shallowRef, watch } from 'vue';
import type { DockviewApi, DockviewReadyEvent } from 'dockview-vue';
import { t } from '../../i18n';
import { markSeen, sessionKey, usePacer, type SessionNode, type SessionRef, type WorktreeNode } from '../pacer';
import { FILE_PANEL_PREFIX, fileNameOf, filePanelId, isFilePanel, normalizePath } from './markdownFiles';
import { sessionPaths, type SessionPath } from './nav';

export const METRICS_PANEL = 'metrics';
const RESERVED_PANELS = new Set([METRICS_PANEL]);

type AddPanel = Parameters<DockviewApi['addPanel']>[0];
type PanelPosition = { referencePanel: string; direction: 'within' | 'right' | 'below' | 'left' };

type Options = {
  position?: (session: SessionNode, api: DockviewApi, paths: ReadonlyMap<string, SessionPath>) => PanelPosition | undefined;
};

let carried: { keys: string[]; active: string | null } = { keys: [], active: null };

export function useAgentsDock(options: Options = {}) {
  const pacer = usePacer();
  const dock = shallowRef<DockviewApi | null>(null);
  const activeKey = shallowRef<string | null>(null);
  const visible = shallowRef(true);
  const creatingIn = shallowRef<string | null>(null);
  const openingKey = shallowRef<string | null>(null);
  let activeChange: { dispose(): void } | null = null;

  const paths = computed(() => sessionPaths(pacer?.tree.value ?? []));
  const activePath = computed(() => (activeKey.value ? paths.value.get(activeKey.value) : undefined));
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
    const session = activeKey.value ? pacer?.sessions.value.get(activeKey.value) : undefined;
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
      for (const panel of api.panels) if (!RESERVED_PANELS.has(panel.id) && !isFilePanel(panel.id) && !sessions.has(panel.id)) panel.api.close();
      const opening = openingKey.value ? sessions.get(openingKey.value) : undefined;
      if (!opening) return;
      openingKey.value = null;
      open(opening);
    },
  );

  function openPanel(panel: AddPanel) {
    const api = dock.value;
    if (!api) return;
    const existing = api.getPanel(panel.id);
    if (existing) {
      existing.api.setActive();
      return;
    }
    api.addPanel(panel);
  }

  function open(session: SessionNode) {
    const api = dock.value;
    if (!api) return;
    const position = options.position?.(session, api, paths.value);
    openPanel({
      id: session.key,
      component: 'session',
      title: session.name,
      params: { session: session.ref },
      renderer: 'always',
      ...(position ? { position } : {}),
    });
  }

  function openFile(path: string, options: { hash?: string; worktreeId?: string; from?: string } = {}) {
    const api = dock.value;
    if (!api) return;
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
    let position: PanelPosition | undefined = anchor ? { referencePanel: anchor, direction: 'within' } : undefined;
    if (!position) {
      const last = api.panels.filter((panel) => !RESERVED_PANELS.has(panel.id)).at(-1);
      if (last) position = { referencePanel: last.id, direction: 'within' };
    }
    api.addPanel({ id, component: 'markdown', title: fileNameOf(path), params: { path: normalizePath(path), hash }, renderer: 'always', ...(position ? { position } : {}) });
  }

  function openMetrics() {
    openPanel({ id: METRICS_PANEL, component: 'metrics', title: t('agents.metrics.title') });
  }

  function openWhenListed(session: SessionRef) {
    const key = sessionKey(session);
    const node = pacer?.sessions.value.get(key);
    if (node) open(node);
    else openingKey.value = key;
  }

  function onCreated(session: SessionRef) {
    creatingIn.value = null;
    openWhenListed(session);
  }

  function onReady({ api }: DockviewReadyEvent) {
    dock.value = api;
    activeChange = api.onDidActivePanelChange(({ panel }) => {
      activeKey.value = panel?.id ?? null;
    });
    const sessions = pacer?.sessions.value;
    for (const key of carried.keys) {
      if (key === METRICS_PANEL) openMetrics();
      else if (isFilePanel(key)) openFile(key.slice(FILE_PANEL_PREFIX.length));
      else {
        const node = sessions?.get(key);
        if (node) open(node);
      }
    }
    if (carried.active) api.getPanel(carried.active)?.api.setActive();
  }

  onActivated(() => {
    visible.value = true;
  });
  onDeactivated(() => {
    visible.value = false;
  });
  onBeforeUnmount(() => {
    const api = dock.value;
    if (api) carried = { keys: api.panels.map((panel) => panel.id), active: activeKey.value };
    activeChange?.dispose();
  });

  return { pacer, dock, activeKey, visible, paths, activePath, creatingIn, creatingWorktree, activeWorktree, open, openFile, openMetrics, openWhenListed, onCreated, onReady };
}
