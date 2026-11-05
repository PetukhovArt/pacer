import { invoke } from '@tauri-apps/api/core';
import { shallowRef, type ShallowRef } from 'vue';
import { describeError } from './api';

export type AiKind = 'claude' | 'codex' | 'gemini' | 'anthropic_api' | 'openai_api' | 'openrouter' | 'grok';
export type AiStatus = 'signed_in' | 'signed_out' | 'not_installed' | 'rejected' | 'unreachable' | 'error' | 'unchecked';
type AiPending = 'login' | 'logout';

export type AiView = {
  kind: AiKind;
  label: string;
  method: 'cli' | 'key';
  status: AiStatus;
  account: string | null;
  plan: string | null;
  detail: string | null;
  version: string | null;
  expires_at: string | null;
  command: string | null;
};

const POLL_INTERVAL_MS = 3000;
const POLL_LIMIT_MS = 120_000;

const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

type AiStore = {
  accounts: ShallowRef<AiView[]>;
  pending: ShallowRef<Partial<Record<AiKind, AiPending>>>;
  problems: ShallowRef<Partial<Record<AiKind, string>>>;
  checking: ShallowRef<boolean>;
  refresh: () => Promise<void>;
  check: () => Promise<void>;
  recheck: (kind: AiKind) => Promise<void>;
  login: (kind: AiKind) => Promise<void>;
  logout: (kind: AiKind) => Promise<void>;
  cancel: (kind: AiKind) => void;
  saveKey: (kind: AiKind, key: string) => Promise<void>;
  removeKey: (kind: AiKind) => Promise<void>;
};

let store: AiStore | null = null;

function createStore(): AiStore {
  const accounts = shallowRef<AiView[]>([]);
  const pending = shallowRef<Partial<Record<AiKind, AiPending>>>({});
  const problems = shallowRef<Partial<Record<AiKind, string>>>({});
  const checking = shallowRef(false);

  const setProblem = (kind: AiKind, message: string | null) => {
    const next = { ...problems.value };
    if (message) next[kind] = message;
    else delete next[kind];
    problems.value = next;
  };

  const setPending = (kind: AiKind, value: AiPending | null) => {
    const next = { ...pending.value };
    if (value) next[kind] = value;
    else delete next[kind];
    pending.value = next;
  };

  const replace = (view: AiView) => {
    const known = accounts.value.some((a) => a.kind === view.kind);
    accounts.value = known ? accounts.value.map((a) => (a.kind === view.kind ? view : a)) : [...accounts.value, view];
  };

  async function list() {
    accounts.value = await invoke<AiView[]>('ai_list');
  }

  async function check() {
    if (checking.value) return;
    checking.value = true;
    try {
      accounts.value = await invoke<AiView[]>('ai_check');
    } catch (e) {
      for (const a of accounts.value) setProblem(a.kind, describeError(e));
    } finally {
      checking.value = false;
    }
  }

  async function refresh() {
    try {
      await list();
    } catch {
      accounts.value = [];
    }
    await check();
  }

  async function recheck(kind: AiKind) {
    setProblem(kind, null);
    try {
      replace(await invoke<AiView>('ai_refresh', { kind }));
    } catch (e) {
      setProblem(kind, describeError(e));
    }
  }

  async function poll(kind: AiKind, mode: AiPending) {
    const goal: AiStatus = mode === 'login' ? 'signed_in' : 'signed_out';
    const started = Date.now();
    while (pending.value[kind] === mode && Date.now() - started < POLL_LIMIT_MS) {
      await pause(POLL_INTERVAL_MS);
      if (pending.value[kind] !== mode) return;
      try {
        const view = await invoke<AiView>('ai_refresh', { kind });
        replace(view);
        if (view.status === goal) break;
      } catch (e) {
        setProblem(kind, describeError(e));
        break;
      }
    }
    if (pending.value[kind] === mode) setPending(kind, null);
  }

  async function launch(kind: AiKind, mode: AiPending) {
    if (pending.value[kind]) return;
    setProblem(kind, null);
    setPending(kind, mode);
    try {
      await invoke<void>(mode === 'login' ? 'ai_login' : 'ai_logout', { kind });
    } catch (e) {
      setProblem(kind, describeError(e));
      setPending(kind, null);
      return;
    }
    void poll(kind, mode);
  }

  async function saveKey(kind: AiKind, key: string) {
    replace(await invoke<AiView>('ai_key_set', { kind, key }));
    setProblem(kind, null);
  }

  async function removeKey(kind: AiKind) {
    try {
      await invoke<void>('ai_key_remove', { kind });
    } catch (e) {
      setProblem(kind, describeError(e));
      return;
    }
    await recheck(kind);
  }

  return {
    accounts,
    pending,
    problems,
    checking,
    refresh,
    check,
    recheck,
    login: (kind) => launch(kind, 'login'),
    logout: (kind) => launch(kind, 'logout'),
    cancel: (kind) => setPending(kind, null),
    saveKey,
    removeKey,
  };
}

export function useAiAccounts(): AiStore {
  store ??= createStore();
  return store;
}
