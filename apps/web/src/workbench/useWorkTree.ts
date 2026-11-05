import { computed, shallowRef, watch } from 'vue';
import { useClientState } from '@core/vue-bindings';
import { usePacer } from '../agents/pacer';
import { useMyIssues } from '../atlassian/myIssues';
import { useConnectors } from '../connectors/api';
import { useForgeInbox } from '../forge/inbox';
import { useSettings } from '../settings/store';
import { lookupRemotes, remotes } from './remotes';
import { buildTree, type Sources, type TreeMode } from './tree';

const MODE_KEY = 'veyra.tree.mode';

export const treeQuery = shallowRef('');
export const treeMode = shallowRef<TreeMode>(localStorage.getItem(MODE_KEY) === 'type' ? 'type' : 'project');

watch(treeMode, (mode) => localStorage.setItem(MODE_KEY, mode));

export function useWorkTree() {
  const pacer = usePacer();
  const state = useClientState();
  const { connectors } = useConnectors();
  const inbox = useForgeInbox();
  const jira = useMyIssues();
  const { values } = useSettings();

  watch(
    () => [...(pacer?.entities.value.projects.values() ?? [])].map((project) => project.repo_path),
    (repos) => lookupRemotes(repos),
    { immediate: true },
  );
  watch(jira.jiraConnectors, () => jira.refresh(false), { immediate: true });

  const sources = computed<Sources>(() => ({
    pacer: pacer !== null,
    relay: state.value.connected,
    workspaces: pacer?.tree.value ?? [],
    entities: pacer?.entities.value,
    remotes: remotes.value,
    connectors: connectors.value,
    mrs: inbox.items.value,
    issues: jira.issues.value,
    tasks: state.value.tasks.filter((task) => task.assigneeId === state.value.me.id && task.statusCategory !== 'done'),
    topics: state.value.topics,
    jiraProjects: values.value['tree.jiraProjects'],
  }));

  const nodes = computed(() => buildTree(sources.value, treeMode.value, treeQuery.value));
  const all = computed(() => buildTree(sources.value, 'project', ''));

  function refresh() {
    void inbox.load();
    jira.refresh(true);
  }

  return { pacer, nodes, all, refresh, loading: computed(() => inbox.loading.value || jira.loading.value), errors: computed(() => [...inbox.errors.value.map((e) => ({ connector: e.connector, message: e.message })), ...jira.errors.value]) };
}
