import { computed, onActivated, onBeforeUnmount, onDeactivated, shallowRef, watch } from 'vue';
import type { DockviewApi, DockviewReadyEvent } from 'dockview-vue';
import { markSeen, sessionKey, usePacer, type SessionNode, type SessionRef, type WorktreeNode } from '../pacer';
import { sessionPaths, type SessionPath } from './nav';

export const METRICS_PANEL = 'metrics';
export const NAVIGATOR_PANEL = 'navigator';
const RESERVED_PANELS = new Set([METRICS_PANEL, NAVIGATOR_PANEL]);

type AddPanel = Parameters<DockviewApi['addPanel']>[0];
type PanelPosition = { referencePanel: string; direction: 'within' | 'right' | 'below' | 'left' };

type Options = {
  position?: (session: SessionNode, api: DockviewApi, paths: ReadonlyMap<string, SessionPath>) => PanelPosition | undefined;
  init?: (api: DockviewApi) => void;
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
  const creatingWorktree = computed<WorktreeNode | null>(() => {
    const id = creatingIn.value;
    if (!id) return null;
    for (const workspace of pacer?.tree.value ?? []) for (const project of workspace.projects) for (const worktree of project.worktrees) if (worktree.id === id) return worktree;
    return null;
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
      for (const panel of api.panels) if (!RESERVED_PANELS.has(panel.id) && !sessions.has(panel.id)) panel.api.close();
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

  function openMetrics() {
    openPanel({ id: METRICS_PANEL, component: 'metrics', title: 'Память' });
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
    options.init?.(api);
    const sessions = pacer?.sessions.value;
    for (const key of carried.keys) {
      if (key === METRICS_PANEL) openMetrics();
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
    if (api) carried = { keys: api.panels.filter((panel) => panel.id !== NAVIGATOR_PANEL).map((panel) => panel.id), active: activeKey.value };
    activeChange?.dispose();
  });

  return { pacer, dock, activeKey, visible, paths, activePath, creatingIn, creatingWorktree, open, openMetrics, openWhenListed, onCreated, onReady };
}
