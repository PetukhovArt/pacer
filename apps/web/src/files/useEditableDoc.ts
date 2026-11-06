import { shallowRef } from 'vue';
import { Text } from '@codemirror/state';
import type { EditorView, ViewUpdate } from '@codemirror/view';
import { describeError } from '../connectors/api';
import { useSettings } from '../settings/store';
import { CHANGED_ON_DISK, fsReadDoc, fsWatch, fsWriteText, type FileDoc } from './api';
import { registerDoc, setDirtyPath, stashDraft, takeDraft } from './documents';
import { replaceDoc } from './editor';

const PREVIEW_DELAY = 150;
const AUTOSAVE_DELAY = 1000;

export type Disk = { text: string; signature: string; crlf: boolean };

type Options = {
  path: () => string;
  view: () => EditorView | undefined;
  reset: (text: string) => void;
  onReplaced: () => void;
};

const normalize = (text: string) => text.replace(/\r\n?/g, '\n');
const serialize = (text: string, crlf: boolean) => (crlf ? text.replace(/\n/g, '\r\n') : text);

function detectCrlf(text: string): boolean {
  const crlf = text.match(/\r\n/g)?.length ?? 0;
  const lf = text.match(/(?<!\r)\n/g)?.length ?? 0;
  return crlf > lf;
}

const toDisk = (doc: FileDoc): Disk => ({ text: normalize(doc.text), signature: doc.signature, crlf: detectCrlf(doc.text) });

export function useEditableDoc(options: Options) {
  const { values } = useSettings();
  const previewText = shallowRef<string | null>(null);
  const failure = shallowRef<string | null>(null);
  const saveError = shallowRef<string | null>(null);
  const dirty = shallowRef(false);
  const saving = shallowRef(false);
  const loaded = shallowRef(false);
  const conflict = shallowRef<Disk | null>(null);

  let savedDoc = Text.empty;
  let signature = '';
  let crlf = false;
  let generation = 0;
  let currentPath = '';
  let silent = false;
  let inflight: string | null = null;
  let running: Promise<boolean> | null = null;
  let unwatch: (() => void) | null = null;
  let unregister: (() => void) | null = null;
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  let autoTimer: ReturnType<typeof setTimeout> | undefined;

  function markDirty(value: boolean) {
    dirty.value = value;
    setDirtyPath(currentPath, value);
  }

  function showPreview() {
    clearTimeout(previewTimer);
    const view = options.view();
    if (view) previewText.value = view.state.doc.toString();
  }

  function onUpdate(update: ViewUpdate) {
    if (silent || !update.docChanged) return;
    markDirty(!update.state.doc.eq(savedDoc));
    clearTimeout(previewTimer);
    previewTimer = setTimeout(showPreview, PREVIEW_DELAY);
    clearTimeout(autoTimer);
    if (values.value['files.autoSave'] === 'afterDelay') autoTimer = setTimeout(() => void save(), AUTOSAVE_DELAY);
  }

  function blurred() {
    if (values.value['files.autoSave'] === 'onBlur' && dirty.value) void save();
  }

  function adopt(disk: Disk, replace: boolean, addToHistory = false) {
    const view = options.view();
    if (!view) return;
    signature = disk.signature;
    crlf = disk.crlf;
    conflict.value = null;
    silent = true;
    try {
      if (replace) replaceDoc(view, disk.text, addToHistory);
    } finally {
      silent = false;
    }
    savedDoc = view.state.doc;
    markDirty(false);
    showPreview();
    options.onReplaced();
  }

  function incoming(raw: FileDoc) {
    const view = options.view();
    if (!view || raw.signature === signature) return;
    const disk = toDisk(raw);
    if (inflight !== null && disk.text === inflight) {
      signature = disk.signature;
      return;
    }
    if (disk.text === view.state.doc.toString()) adopt(disk, false);
    else if (!dirty.value && !saving.value) adopt(disk, true);
    else conflict.value = disk;
  }

  async function raiseConflict() {
    try {
      incomingFromWrite(await fsReadDoc(currentPath));
    } catch (e) {
      saveError.value = describeError(e);
    }
  }

  function incomingFromWrite(raw: FileDoc) {
    const view = options.view();
    if (!view) return;
    const disk = toDisk(raw);
    if (disk.text === view.state.doc.toString()) adopt(disk, false);
    else conflict.value = disk;
  }

  async function write(): Promise<boolean> {
    const view = options.view();
    if (!view || !dirty.value) return true;
    if (conflict.value) return false;
    saving.value = true;
    saveError.value = null;
    const snapshot = view.state.doc;
    const text = snapshot.toString();
    inflight = text;
    try {
      signature = await fsWriteText(currentPath, serialize(text, crlf), signature);
      savedDoc = snapshot;
      markDirty(!view.state.doc.eq(savedDoc));
      return true;
    } catch (e) {
      const message = describeError(e);
      if (message.includes(CHANGED_ON_DISK)) await raiseConflict();
      else saveError.value = message;
      return false;
    } finally {
      saving.value = false;
      inflight = null;
    }
  }

  function save(): Promise<boolean> {
    clearTimeout(autoTimer);
    if (running) return running.then(() => save());
    const attempt = write().finally(() => {
      running = null;
    });
    running = attempt;
    return attempt;
  }

  function reload() {
    const disk = conflict.value;
    if (disk) adopt(disk, true, true);
  }

  async function keepMine() {
    const disk = conflict.value;
    if (!disk) return;
    signature = disk.signature;
    crlf = disk.crlf;
    conflict.value = null;
    await save();
  }

  function discard() {
    clearTimeout(autoTimer);
    const view = options.view();
    if (view) savedDoc = view.state.doc;
    conflict.value = null;
    markDirty(false);
  }

  function mine(): string {
    return options.view()?.state.doc.toString() ?? '';
  }

  async function load() {
    const ticket = ++generation;
    unwatch?.();
    unwatch = null;
    unregister?.();
    failure.value = null;
    saveError.value = null;
    conflict.value = null;
    loaded.value = false;
    previewText.value = null;
    currentPath = options.path();
    unregister = registerDoc(currentPath, { save, discard });
    try {
      const raw = await fsReadDoc(currentPath);
      if (ticket !== generation) return;
      const draft = takeDraft(currentPath);
      if (draft) {
        crlf = draft.crlf;
        signature = draft.signature;
        options.reset(draft.text);
        savedDoc = Text.of(draft.saved.split('\n'));
        markDirty(!(options.view()?.state.doc.eq(savedDoc) ?? true));
      } else {
        const disk = toDisk(raw);
        crlf = disk.crlf;
        signature = disk.signature;
        options.reset(disk.text);
        savedDoc = options.view()?.state.doc ?? Text.empty;
        markDirty(false);
      }
      showPreview();
      loaded.value = true;
      if (draft) incoming(raw);
      const stop = await fsWatch(currentPath, (next) => {
        if (ticket === generation) incoming(next);
      });
      if (ticket === generation) unwatch = stop;
      else stop();
    } catch (e) {
      if (ticket === generation) failure.value = describeError(e);
    }
  }

  function dispose() {
    generation++;
    unwatch?.();
    unwatch = null;
    clearTimeout(previewTimer);
    clearTimeout(autoTimer);
    const view = options.view();
    unregister?.();
    unregister = null;
    if (!dirty.value || !view || !loaded.value) {
      setDirtyPath(currentPath, false);
      return;
    }
    const path = currentPath;
    const draft = { text: view.state.doc.toString(), saved: savedDoc.toString(), signature, crlf };
    stashDraft(path, draft);
    const forget = registerDoc(path, {
      save: async () => {
        try {
          await fsWriteText(path, serialize(draft.text, draft.crlf), draft.signature);
        } catch {
          return false;
        }
        takeDraft(path);
        setDirtyPath(path, false);
        forget();
        return true;
      },
      discard: () => {
        takeDraft(path);
        setDirtyPath(path, false);
        forget();
      },
    });
  }

  return { previewText, failure, saveError, dirty, saving, loaded, conflict, load, dispose, save, reload, keepMine, discard, mine, onUpdate, blurred };
}
