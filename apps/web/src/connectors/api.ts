import { invoke } from '@tauri-apps/api/core';
import { shallowRef, type ShallowRef } from 'vue';

export type ConnectorKind = 'github' | 'gitlab' | 'jira' | 'confluence';
type ConnectorAuth = 'cli' | 'token';
export type ConnectorStatus = 'ok' | 'error' | 'unreachable' | 'unchecked';

export type Person = { name: string; username: string; avatar_url: string | null };

export type ConnectorView = {
  id: string;
  kind: ConnectorKind;
  label: string;
  base_url: string;
  auth: ConnectorAuth;
  account: Person | null;
  status: ConnectorStatus;
  error: string | null;
};

export type Detected = { kind: ConnectorKind; base_url: string; account: string | null };

export type PipelineStatus = 'success' | 'failed' | 'running' | 'pending' | 'canceled' | 'none';

export type ChangeRequestSummary = {
  connector: string;
  project: string;
  number: number;
  title: string;
  url: string;
  state: 'open' | 'merged' | 'closed';
  draft: boolean;
  author: Person;
  source_branch: string;
  target_branch: string;
  updated_at: string;
  pipeline: PipelineStatus;
  unresolved_threads: number | null;
};

type InboxReason = 'author' | 'reviewer' | 'assignee';
export type InboxItem = ChangeRequestSummary & { reasons: InboxReason[] };
export type InboxError = { connector: string; message: string };
export type Inbox = { items: InboxItem[]; errors: InboxError[] };

export type ChangeRequest = ChangeRequestSummary & {
  description: string;
  reviewers: Person[];
  labels: string[];
  created_at: string;
  mergeable: boolean | null;
};

export type DiffFile = {
  old_path: string;
  new_path: string;
  status: 'added' | 'deleted' | 'modified' | 'renamed';
  patch: string;
  additions: number;
  deletions: number;
  binary: boolean;
  too_large: boolean;
};

type DiscussionNote = { id: string; author: Person; body: string; created_at: string; system: boolean };
type DiscussionPosition = { path: string; new_line: number | null; old_line: number | null };
export type Discussion = {
  id: string;
  resolvable: boolean;
  resolved: boolean;
  position: DiscussionPosition | null;
  notes: DiscussionNote[];
};

export type PipelineJob = { id: string; name: string; stage: string; status: string; url: string; duration_s: number | null };
export type Pipeline = { id: string; status: PipelineStatus; url: string; jobs: PipelineJob[] };

export type JiraIssueSummary = {
  connector: string;
  key: string;
  summary: string;
  status: string;
  status_category: 'todo' | 'in_progress' | 'done';
  priority: string | null;
  issue_type: string;
  assignee: Person | null;
  updated: string;
  url: string;
};

export type JiraIssue = JiraIssueSummary & {
  description_html: string;
  reporter: Person | null;
  labels: string[];
  fix_versions: string[];
  components: string[];
  comments: { id: string; author: Person; body_html: string; created: string }[];
  links: { relation: string; key: string; summary: string; status: string }[];
  subtasks: JiraIssueSummary[];
};

export type ConfluenceHit = { connector: string; id: string; title: string; space: string; url: string; excerpt: string; updated: string };
export type ConfluencePage = {
  connector: string;
  id: string;
  title: string;
  space: string;
  url: string;
  version: number;
  body_html: string;
  ancestors: { id: string; title: string }[];
};

export type ChangeRequestRef = { connector: string; project: string; number: number };

type ConnectorAddInput = {
  kind: ConnectorKind;
  baseUrl: string;
  label: string;
  auth: ConnectorAuth;
  token: string | null;
  email: string | null;
};

type ConnectorUpdateInput = { id: string; label: string; token: string | null; email: string | null };

export function describeError(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export async function openUrl(url: string): Promise<void> {
  try {
    await invoke<void>('plugin:opener|open_url', { url });
  } catch {
    window.open(url, '_blank', 'noopener');
  }
}

export const connectorsList =() => invoke<ConnectorView[]>('connectors_list');
const connectorsCheck = () => invoke<ConnectorView[]>('connectors_check');
const connectorsDetect = () => invoke<Detected[]>('connectors_detect');
const connectorAdd = (input: ConnectorAddInput) => invoke<ConnectorView>('connector_add', { ...input });
const connectorUpdate = (input: ConnectorUpdateInput) => invoke<ConnectorView>('connector_update', { ...input });
const connectorRemove = (id: string) => invoke<void>('connector_remove', { id });
export const connectorAsset = (id: string, url: string) => invoke<{ mime: string; base64: string }>('connector_asset', { id, url });

export const forgeInbox = () => invoke<Inbox>('forge_inbox');
export const forgeForWorktree = (path: string, branch: string) => invoke<ChangeRequestSummary | null>('forge_for_worktree', { path, branch });
export const forgeChangeRequest = (ref: ChangeRequestRef) => invoke<ChangeRequest>('forge_change_request', { ...ref });
export const forgeDiff = (ref: ChangeRequestRef) => invoke<DiffFile[]>('forge_diff', { ...ref });
export const forgeDiscussions = (ref: ChangeRequestRef) => invoke<Discussion[]>('forge_discussions', { ...ref });
export const forgePipeline = (ref: ChangeRequestRef) => invoke<Pipeline | null>('forge_pipeline', { ...ref });

export const jiraSearch = (connector: string, jql: string, start: number, limit: number) =>
  invoke<{ total: number; issues: JiraIssueSummary[] }>('jira_search', { connector, jql, start, limit });
export const jiraIssue = (connector: string, key: string) => invoke<JiraIssue>('jira_issue', { connector, key });

export const confluenceSearch = (connector: string, text: string, limit: number) => invoke<ConfluenceHit[]>('confluence_search', { connector, text, limit });
export const confluencePage = (connector: string, id: string) => invoke<ConfluencePage>('confluence_page', { connector, id });

type ConnectorsStore = {
  connectors: ShallowRef<ConnectorView[]>;
  detected: ShallowRef<Detected[]>;
  loading: ShallowRef<boolean>;
  checking: ShallowRef<boolean>;
  error: ShallowRef<string | null>;
  list: () => Promise<void>;
  check: () => Promise<void>;
  detect: () => Promise<void>;
  add: (input: ConnectorAddInput) => Promise<ConnectorView>;
  update: (input: ConnectorUpdateInput) => Promise<ConnectorView>;
  remove: (id: string) => Promise<void>;
  refresh: () => Promise<void>;
};

let store: ConnectorsStore | null = null;

function createStore(): ConnectorsStore {
  const connectors = shallowRef<ConnectorView[]>([]);
  const detected = shallowRef<Detected[]>([]);
  const loading = shallowRef(false);
  const checking = shallowRef(false);
  const error = shallowRef<string | null>(null);

  async function guarded(flag: ShallowRef<boolean>, operation: () => Promise<void>) {
    if (flag.value) return;
    flag.value = true;
    try {
      await operation();
      error.value = null;
    } catch (e) {
      error.value = describeError(e);
    } finally {
      flag.value = false;
    }
  }

  const list = () =>
    guarded(loading, async () => {
      connectors.value = await connectorsList();
    });

  const check = () =>
    guarded(checking, async () => {
      connectors.value = await connectorsCheck();
    });

  async function detect() {
    try {
      detected.value = await connectorsDetect();
    } catch {
      detected.value = [];
    }
  }

  function upsert(view: ConnectorView) {
    const known = connectors.value.some((c) => c.id === view.id);
    connectors.value = known ? connectors.value.map((c) => (c.id === view.id ? view : c)) : [...connectors.value, view];
  }

  async function add(input: ConnectorAddInput) {
    const view = await connectorAdd(input);
    upsert(view);
    await detect();
    return view;
  }

  async function update(input: ConnectorUpdateInput) {
    const view = await connectorUpdate(input);
    upsert(view);
    return view;
  }

  async function remove(id: string) {
    await connectorRemove(id);
    connectors.value = connectors.value.filter((c) => c.id !== id);
    await detect();
  }

  async function refresh() {
    await list();
    await Promise.all([detect(), check()]);
  }

  return { connectors, detected, loading, checking, error, list, check, detect, add, update, remove, refresh };
}

export function useConnectors(): ConnectorsStore {
  store ??= createStore();
  return store;
}
