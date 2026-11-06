import './actions';
import { readContext } from './context';
import { chordFromEvent, isTypingChord, type Chord } from './keys';
import { resolveSequence, startKeymap } from './keymap';
import { getAction, isAvailable, runAction } from './registry';

const SEQUENCE_TIMEOUT_MS = 1500;

type Capture = (chord: Chord) => void;

let capture: Capture | null = null;
let pending: Chord[] = [];
let timer: ReturnType<typeof setTimeout> | undefined;
let installed = false;

export function captureChords(handler: Capture): () => void {
  capture = handler;
  return () => {
    if (capture === handler) capture = null;
  };
}

function swallow(event: KeyboardEvent): void {
  event.preventDefault();
  event.stopPropagation();
}

function clearPending(): void {
  pending = [];
  clearTimeout(timer);
}

function exactAction(chords: readonly Chord[], context: ReturnType<typeof readContext>): string | null {
  const { exact } = resolveSequence(context, chords);
  const action = exact === null ? undefined : getAction(exact);
  return action && isAvailable(action, context) ? action.id : null;
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.isComposing) return;
  const chord = chordFromEvent(event);
  if (!chord) return;
  if (capture) {
    swallow(event);
    capture(chord);
    return;
  }
  const context = readContext(event.target);
  if (isTypingChord(chord) && (context.inputFocus || context.terminalFocus)) {
    clearPending();
    return;
  }
  if (pending.length && chord.code === 'Escape' && !chord.ctrl && !chord.alt && !chord.shift && !chord.meta) {
    clearPending();
    swallow(event);
    return;
  }
  for (const sequence of pending.length ? [[...pending, chord], [chord]] : [[chord]]) {
    const { longer } = resolveSequence(context, sequence);
    const action = exactAction(sequence, context);
    if (longer) {
      swallow(event);
      clearTimeout(timer);
      pending = sequence;
      timer = setTimeout(() => {
        pending = [];
        if (action) runAction(action);
      }, SEQUENCE_TIMEOUT_MS);
      return;
    }
    if (action) {
      clearPending();
      swallow(event);
      runAction(action, context);
      return;
    }
  }
  if (pending.length) {
    clearPending();
    swallow(event);
  }
}

export function installKeymap(): void {
  if (installed) return;
  installed = true;
  startKeymap();
  window.addEventListener('keydown', onKeyDown, true);
}
