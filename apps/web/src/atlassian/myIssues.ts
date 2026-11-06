import { computed, shallowRef } from 'vue';
import { describeError, jiraSearch, onConnectorRecovered, useConnectors, type JiraIssueSummary } from '../connectors/api';

const JIRA_JQL = 'assignee = currentUser() AND resolution = Unresolved ORDER BY updated DESC';
const JIRA_LIMIT = 100;
const JIRA_FRESH_MS = 60_000;

type JiraResult = { issues: JiraIssueSummary[]; error: string | null; loading: boolean; loadedAt: number };

const jiraResults = shallowRef<ReadonlyMap<string, JiraResult>>(new Map());

function patchJira(connector: string, patch: Partial<JiraResult>) {
  const previous = jiraResults.value.get(connector) ?? { issues: [], error: null, loading: false, loadedAt: 0 };
  jiraResults.value = new Map([...jiraResults.value, [connector, { ...previous, ...patch }]]);
}

async function loadJira(connector: string, force: boolean) {
  const current = jiraResults.value.get(connector);
  if (current?.loading || (!force && current && Date.now() - current.loadedAt < JIRA_FRESH_MS)) return;
  patchJira(connector, { loading: true });
  try {
    const result = await jiraSearch(connector, JIRA_JQL, 0, JIRA_LIMIT);
    patchJira(connector, { issues: result.issues, error: null, loading: false, loadedAt: Date.now() });
  } catch (e) {
    patchJira(connector, { error: describeError(e), loading: false, loadedAt: Date.now() });
  }
}

let watching = false;

export function useMyIssues() {
  if (!watching) {
    watching = true;
    onConnectorRecovered((id) => {
      if (jiraResults.value.get(id)?.error) void loadJira(id, true);
    });
  }
  const { connectors } = useConnectors();
  const jiraConnectors = computed(() => connectors.value.filter((c) => c.kind === 'jira'));
  const issues = computed(() => jiraConnectors.value.flatMap((c) => jiraResults.value.get(c.id)?.issues ?? []));
  const errors = computed(() =>
    jiraConnectors.value.flatMap((c) => {
      const error = jiraResults.value.get(c.id)?.error;
      return error ? [{ connector: c.label, message: error }] : [];
    }),
  );
  const loading = computed(() => jiraConnectors.value.some((c) => jiraResults.value.get(c.id)?.loading));

  function refresh(force: boolean) {
    for (const connector of jiraConnectors.value) void loadJira(connector.id, force);
  }

  return { jiraConnectors, issues, errors, loading, refresh };
}
