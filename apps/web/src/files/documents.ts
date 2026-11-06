import { shallowRef } from 'vue';

export type DocHandle = { save: () => Promise<boolean>; discard: () => void };
type Choice = 'save' | 'discard' | 'cancel';
export type UnsavedPrompt = { paths: string[]; settle: (choice: Choice) => void };

const handles = new Map<string, DocHandle>();

const dirtyPaths = shallowRef<ReadonlySet<string>>(new Set());
export const unsavedPrompt = shallowRef<UnsavedPrompt | null>(null);

export const isDirtyPath = (path: string) => dirtyPaths.value.has(path);

export function setDirtyPath(path: string, dirty: boolean): void {
  if (dirtyPaths.value.has(path) === dirty) return;
  const next = new Set(dirtyPaths.value);
  if (dirty) next.add(path);
  else next.delete(path);
  dirtyPaths.value = next;
}

export function registerDoc(path: string, handle: DocHandle): () => void {
  handles.set(path, handle);
  return () => {
    if (handles.get(path) === handle) handles.delete(path);
  };
}

function ask(paths: string[]): Promise<Choice> {
  return new Promise((resolve) => {
    unsavedPrompt.value = {
      paths,
      settle: (choice) => {
        unsavedPrompt.value = null;
        resolve(choice);
      },
    };
  });
}

export async function settleDirty(paths: string[]): Promise<boolean> {
  const dirty = paths.filter(isDirtyPath);
  if (!dirty.length) return true;
  if (unsavedPrompt.value) return false;
  const choice = await ask(dirty);
  if (choice === 'cancel') return false;
  if (choice === 'discard') {
    for (const path of dirty) handles.get(path)?.discard();
    return true;
  }
  const saved = await Promise.all(dirty.map((path) => handles.get(path)?.save() ?? Promise.resolve(true)));
  return saved.every(Boolean);
}

export const settleAll = () => settleDirty([...dirtyPaths.value]);

window.addEventListener('beforeunload', (event) => {
  if (!dirtyPaths.value.size) return;
  event.preventDefault();
  event.returnValue = '';
});

type Draft = { text: string; saved: string; signature: string; crlf: boolean };

const drafts = new Map<string, Draft>();

export const stashDraft = (path: string, draft: Draft) => void drafts.set(path, draft);

export function takeDraft(path: string): Draft | undefined {
  const draft = drafts.get(path);
  drafts.delete(path);
  return draft;
}
