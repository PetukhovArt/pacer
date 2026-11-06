export type Chord = { readonly ctrl: boolean; readonly alt: boolean; readonly shift: boolean; readonly meta: boolean; readonly code: string };

const NAMED: readonly (readonly [string, string])[] = [
  [',', 'Comma'],
  ['.', 'Period'],
  ['/', 'Slash'],
  [';', 'Semicolon'],
  ["'", 'Quote'],
  ['[', 'BracketLeft'],
  [']', 'BracketRight'],
  ['\\', 'Backslash'],
  ['`', 'Backquote'],
  ['-', 'Minus'],
  ['=', 'Equal'],
  ['space', 'Space'],
  ['enter', 'Enter'],
  ['escape', 'Escape'],
  ['tab', 'Tab'],
  ['backspace', 'Backspace'],
  ['delete', 'Delete'],
  ['insert', 'Insert'],
  ['up', 'ArrowUp'],
  ['down', 'ArrowDown'],
  ['left', 'ArrowLeft'],
  ['right', 'ArrowRight'],
  ['home', 'Home'],
  ['end', 'End'],
  ['pageup', 'PageUp'],
  ['pagedown', 'PageDown'],
];

const ALIASES: Readonly<Record<string, string>> = {
  esc: 'escape',
  return: 'enter',
  del: 'delete',
  comma: ',',
  period: '.',
  dot: '.',
  slash: '/',
  semicolon: ';',
  quote: "'",
  minus: '-',
  plus: '=',
  equal: '=',
  equals: '=',
  backslash: '\\',
  backquote: '`',
  grave: '`',
};

const MODIFIER_NAMES: Readonly<Record<string, 'ctrl' | 'alt' | 'shift' | 'meta'>> = {
  ctrl: 'ctrl',
  control: 'ctrl',
  alt: 'alt',
  option: 'alt',
  shift: 'shift',
  meta: 'meta',
  cmd: 'meta',
  super: 'meta',
  win: 'meta',
};

const MODIFIER_CODES = new Set(['ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'ShiftLeft', 'ShiftRight', 'MetaLeft', 'MetaRight', 'OS']);

const TOKEN_TO_CODE = new Map<string, string>(NAMED);
const CODE_TO_TOKEN = new Map<string, string>(NAMED.map(([token, code]) => [code, token]));
for (let i = 0; i < 26; i++) {
  const letter = String.fromCharCode(97 + i);
  TOKEN_TO_CODE.set(letter, `Key${letter.toUpperCase()}`);
  CODE_TO_TOKEN.set(`Key${letter.toUpperCase()}`, letter);
}
for (let i = 0; i < 10; i++) {
  TOKEN_TO_CODE.set(String(i), `Digit${i}`);
  CODE_TO_TOKEN.set(`Digit${i}`, String(i));
}
for (let i = 1; i <= 12; i++) {
  TOKEN_TO_CODE.set(`f${i}`, `F${i}`);
  CODE_TO_TOKEN.set(`F${i}`, `f${i}`);
}

const DISPLAY: Readonly<Record<string, string>> = {
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Escape: 'Esc',
};

const MODIFIER_ORDER = ['ctrl', 'alt', 'shift', 'meta'] as const;
const MODIFIER_LABELS = { ctrl: 'Ctrl', alt: 'Alt', shift: 'Shift', meta: 'Meta' } as const;
const MODIFIER_PREFIX = /^([a-z]+)-(?=.)/i;

function normalizeCode(code: string): string {
  if (code === 'NumpadAdd') return 'Equal';
  if (code === 'NumpadSubtract') return 'Minus';
  if (code === 'NumpadEnter') return 'Enter';
  if (/^Numpad\d$/.test(code)) return `Digit${code.slice(-1)}`;
  return code;
}

function parseChord(source: string): Chord | null {
  const mods = { ctrl: false, alt: false, shift: false, meta: false };
  let rest = source.trim();
  for (let match = MODIFIER_PREFIX.exec(rest); match; match = MODIFIER_PREFIX.exec(rest)) {
    const name = MODIFIER_NAMES[(match[1] ?? '').toLowerCase()];
    if (!name) break;
    mods[name] = true;
    rest = rest.slice(match[0].length);
  }
  const lower = rest.length > 1 ? rest.toLowerCase() : rest;
  const token = ALIASES[lower] ?? lower;
  const code = TOKEN_TO_CODE.get(token.length === 1 ? token.toLowerCase() : token);
  return code ? { ...mods, code } : null;
}

export function parseSequence(source: string): Chord[] | null {
  const parts = source.trim().split(/\s+/);
  if (!parts[0]) return null;
  const chords: Chord[] = [];
  for (const part of parts) {
    const chord = parseChord(part);
    if (!chord) return null;
    chords.push(chord);
  }
  return chords;
}

export function chordFromEvent(event: KeyboardEvent): Chord | null {
  if (MODIFIER_CODES.has(event.code)) return null;
  const code = normalizeCode(event.code);
  if (!CODE_TO_TOKEN.has(code)) return null;
  return { ctrl: event.ctrlKey, alt: event.altKey, shift: event.shiftKey, meta: event.metaKey, code };
}

function chordKey(chord: Chord): string {
  const mods = MODIFIER_ORDER.filter((name) => chord[name]);
  return [...mods, CODE_TO_TOKEN.get(chord.code) ?? chord.code].join('-');
}

export function sequenceKey(chords: readonly Chord[]): string {
  return chords.map(chordKey).join(' ');
}

function chordLabel(chord: Chord): string {
  const mods = MODIFIER_ORDER.filter((name) => chord[name]).map((name) => MODIFIER_LABELS[name]);
  const token = CODE_TO_TOKEN.get(chord.code) ?? chord.code;
  const key = DISPLAY[chord.code] ?? (token.length === 1 ? token.toUpperCase() : token.charAt(0).toUpperCase() + token.slice(1));
  return [...mods, key].join('+');
}

export function sequenceLabel(chords: readonly Chord[]): string {
  return chords.map(chordLabel).join(' ');
}

export function isTypingChord(chord: Chord): boolean {
  return !chord.ctrl && !chord.alt && !chord.meta && !/^F\d+$/.test(chord.code) && chord.code !== 'Escape';
}
