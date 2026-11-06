import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { languages } from '@codemirror/language-data';
import { bracketMatching, HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { highlightSelectionMatches, search, searchKeymap } from '@codemirror/search';
import { Compartment, EditorSelection, EditorState, Transaction, type Extension } from '@codemirror/state';
import { drawSelection, EditorView, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers, type Command, type ViewUpdate } from '@codemirror/view';
import { tags } from '@lezer/highlight';

export type EditorHooks = {
  onUpdate: (update: ViewUpdate) => void;
  onBlur: () => void;
};

const RU_PHRASES: Record<string, string> = {
  Find: 'Найти',
  Replace: 'Заменить',
  next: 'далее',
  previous: 'назад',
  all: 'все',
  'match case': 'с учётом регистра',
  regexp: 'регулярное выражение',
  'by word': 'слово целиком',
  replace: 'заменить',
  'replace all': 'заменить все',
  close: 'закрыть',
  'current match': 'текущее совпадение',
  'replaced $ matches': 'заменено: $',
  'replaced match on line $': 'заменено на строке $',
  'on line': 'на строке',
  'Go to line': 'Перейти к строке',
  go: 'перейти',
  'Selection deleted': 'Выделение удалено',
};

const theme = EditorView.theme({
  '&': {
    height: '100%',
    color: 'var(--vr-color-text)',
    backgroundColor: 'var(--vr-color-surface)',
    fontSize: 'var(--vr-font-size-xs)',
  },
  '&.cm-focused': { outline: 'none' },
  '.cm-scroller': { fontFamily: 'var(--vr-font-mono)', lineHeight: '20px', overflow: 'auto' },
  '.cm-content': { caretColor: 'var(--vr-color-text)', padding: '0 0 var(--vr-space-4)' },
  '.cm-line': { padding: '0 var(--vr-space-3) 0 var(--vr-space-2)' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--vr-color-text)' },
  '.cm-gutters': { backgroundColor: 'var(--vr-color-surface)', color: 'var(--vr-color-text-muted)', border: 'none' },
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 var(--vr-space-2) 0 var(--vr-space-3)', minWidth: '40px' },
  '.cm-activeLine': { backgroundColor: 'var(--vr-color-nav-hover)' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--vr-color-nav-hover)', color: 'var(--vr-color-text)' },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': { backgroundColor: 'var(--vr-color-nav-active)' },
  '.cm-selectionMatch': { backgroundColor: 'var(--vr-color-nav-hover)' },
  '.cm-searchMatch': { backgroundColor: 'var(--vr-color-nav-active)', outline: '1px solid var(--vr-color-border)' },
  '.cm-searchMatch-selected': { backgroundColor: 'var(--vr-color-accent)', color: 'var(--vr-color-accent-contrast)' },
  '&.cm-focused .cm-matchingBracket': { backgroundColor: 'var(--vr-color-nav-active)' },
  '.cm-panels': {
    backgroundColor: 'var(--vr-color-surface-raised)',
    color: 'var(--vr-color-text)',
    fontFamily: 'var(--vr-font-family)',
    fontSize: 'var(--vr-font-size-xs)',
  },
  '.cm-panels.cm-panels-top': { borderBottom: '1px solid var(--vr-color-border)' },
  '.cm-panels.cm-panels-bottom': { borderTop: '1px solid var(--vr-color-border)' },
  '.cm-panel.cm-search': { padding: 'var(--vr-space-2) var(--vr-space-6) var(--vr-space-2) var(--vr-space-3)' },
  '.cm-panel.cm-search label': { color: 'var(--vr-color-text-muted)' },
  '.cm-textfield': {
    color: 'var(--vr-color-text)',
    backgroundColor: 'var(--vr-color-surface)',
    border: '1px solid var(--vr-color-border)',
    borderRadius: 'var(--vr-radius-sm)',
    fontFamily: 'var(--vr-font-mono)',
  },
  '.cm-button': {
    color: 'var(--vr-color-text)',
    backgroundImage: 'none',
    backgroundColor: 'var(--vr-color-surface)',
    border: '1px solid var(--vr-color-border)',
    borderRadius: 'var(--vr-radius-sm)',
    cursor: 'pointer',
  },
  '.cm-button:hover': { backgroundColor: 'var(--vr-color-nav-hover)' },
  '.cm-panel.cm-search [name=close]': { color: 'var(--vr-color-text-muted)', cursor: 'pointer' },
  '.cm-tooltip': {
    color: 'var(--vr-color-text)',
    backgroundColor: 'var(--vr-color-surface-raised)',
    border: '1px solid var(--vr-color-border)',
  },
});

const highlightStyle = HighlightStyle.define([
  { tag: tags.heading, color: 'var(--vr-hl-title)', fontWeight: '600' },
  { tag: [tags.keyword, tags.operatorKeyword, tags.modifier, tags.controlKeyword], color: 'var(--vr-hl-keyword)' },
  { tag: [tags.string, tags.regexp, tags.special(tags.string)], color: 'var(--vr-hl-string)' },
  { tag: [tags.number, tags.bool, tags.null, tags.atom], color: 'var(--vr-hl-number)' },
  { tag: [tags.comment, tags.quote], color: 'var(--vr-hl-comment)' },
  { tag: [tags.typeName, tags.className, tags.namespace], color: 'var(--vr-hl-type)' },
  { tag: [tags.propertyName, tags.attributeName], color: 'var(--vr-hl-attr)' },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.definition(tags.variableName)], color: 'var(--vr-hl-title)' },
  { tag: [tags.link, tags.url], color: 'var(--vr-hl-string)' },
  { tag: tags.link, textDecoration: 'underline' },
  { tag: [tags.monospace, tags.processingInstruction, tags.contentSeparator, tags.meta], color: 'var(--vr-hl-builtin)' },
  { tag: tags.emphasis, fontStyle: 'italic' },
  { tag: tags.strong, fontWeight: '700' },
  { tag: tags.strikethrough, textDecoration: 'line-through' },
  { tag: tags.invalid, color: 'var(--vr-color-danger)' },
]);

export const baseExtensions: Extension[] = [theme, syntaxHighlighting(highlightStyle), EditorView.lineWrapping];

const toggleWrap =
  (open: string, close = open): Command =>
  (view) => {
    const { doc } = view.state;
    view.dispatch(
      view.state.changeByRange((range) => {
        const before = doc.sliceString(Math.max(0, range.from - open.length), range.from);
        const after = doc.sliceString(range.to, Math.min(doc.length, range.to + close.length));
        if (before === open && after === close) {
          return {
            changes: [
              { from: range.from - open.length, to: range.from },
              { from: range.to, to: range.to + close.length },
            ],
            range: EditorSelection.range(range.from - open.length, range.to - open.length),
          };
        }
        const selected = doc.sliceString(range.from, range.to);
        if (selected.length >= open.length + close.length && selected.startsWith(open) && selected.endsWith(close)) {
          return {
            changes: [
              { from: range.from, to: range.from + open.length },
              { from: range.to - close.length, to: range.to },
            ],
            range: EditorSelection.range(range.from, range.to - open.length - close.length),
          };
        }
        return {
          changes: [
            { from: range.from, insert: open },
            { from: range.to, insert: close },
          ],
          range: range.empty ? EditorSelection.cursor(range.from + open.length) : EditorSelection.range(range.from + open.length, range.to + open.length),
        };
      }),
      { scrollIntoView: true, userEvent: 'input.format' },
    );
    return true;
  };

const insertLink: Command = (view) => {
  view.dispatch(
    view.state.changeByRange((range) => {
      const length = range.to - range.from;
      const url = 'url';
      const changes = [
        { from: range.from, insert: '[' },
        { from: range.to, insert: `](${url})` },
      ];
      if (length === 0) return { changes, range: EditorSelection.cursor(range.from + 1) };
      const start = range.from + 1 + length + 2;
      return { changes, range: EditorSelection.range(start, start + url.length) };
    }),
    { scrollIntoView: true, userEvent: 'input.format' },
  );
  return true;
};

const setHeading =
  (level: number): Command =>
  (view) => {
    const { doc } = view.state;
    const lines = new Set<number>();
    for (const range of view.state.selection.ranges) {
      for (let n = doc.lineAt(range.from).number; n <= doc.lineAt(range.to).number; n++) lines.add(n);
    }
    const prefix = `${'#'.repeat(level)} `;
    const changes = [...lines].map((n) => {
      const line = doc.line(n);
      const existing = /^#{1,6}\s+/.exec(line.text)?.[0] ?? '';
      return { from: line.from, to: line.from + existing.length, insert: existing === prefix ? '' : prefix };
    });
    view.dispatch({ changes, scrollIntoView: true, userEvent: 'input.format' });
    return true;
  };

export const languageSlot = new Compartment();
export const darkSlot = new Compartment();

export function phrases(locale: string): Extension {
  return EditorState.phrases.of(locale === 'ru' ? RU_PHRASES : {});
}

export function createExtensions(hooks: EditorHooks, locale: string, dark: boolean): Extension[] {
  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    history(),
    drawSelection(),
    bracketMatching(),
    highlightSelectionMatches(),
    search({ top: true }),
    markdown({ base: markdownLanguage, codeLanguages: languages }),
    ...baseExtensions,
    languageSlot.of(phrases(locale)),
    darkSlot.of(EditorView.darkTheme.of(dark)),
    keymap.of([
      { key: 'Mod-b', run: toggleWrap('**') },
      { key: 'Mod-i', run: toggleWrap('*') },
      { key: 'Mod-e', run: toggleWrap('`') },
      { key: 'Mod-k', run: insertLink },
      { key: 'Mod-Alt-1', run: setHeading(1) },
      { key: 'Mod-Alt-2', run: setHeading(2) },
      { key: 'Mod-Alt-3', run: setHeading(3) },
      ...searchKeymap,
      ...historyKeymap,
      indentWithTab,
      ...defaultKeymap,
    ]),
    EditorView.updateListener.of((update) => hooks.onUpdate(update)),
    EditorView.domEventHandlers({
      blur: () => {
        hooks.onBlur();
      },
    }),
  ];
}

export function replaceDoc(view: EditorView, text: string, addToHistory: boolean): void {
  const current = view.state.doc.toString();
  if (current === text) return;
  const max = Math.min(current.length, text.length);
  let start = 0;
  while (start < max && current.charCodeAt(start) === text.charCodeAt(start)) start++;
  const isHigh = (code: number) => code >= 0xd800 && code <= 0xdbff;
  if (start > 0 && isHigh(current.charCodeAt(start - 1))) start--;
  let endCurrent = current.length;
  let endText = text.length;
  while (endCurrent > start && endText > start && current.charCodeAt(endCurrent - 1) === text.charCodeAt(endText - 1)) {
    endCurrent--;
    endText--;
  }
  if (endCurrent < current.length && current.charCodeAt(endCurrent) >= 0xdc00 && current.charCodeAt(endCurrent) <= 0xdfff) {
    endCurrent++;
    endText++;
  }
  view.dispatch({
    changes: { from: start, to: endCurrent, insert: text.slice(start, endText) },
    annotations: Transaction.addToHistory.of(addToHistory),
  });
}

export function topLineOf(view: EditorView): number {
  const top = Math.max(0, view.scrollDOM.scrollTop - view.documentPadding.top);
  const block = view.lineBlockAtHeight(top);
  const line = view.state.doc.lineAt(block.from).number - 1;
  const fraction = block.height > 0 ? Math.min(1, Math.max(0, (top - block.top) / block.height)) : 0;
  return line + fraction;
}

export function scrollTopOfLine(view: EditorView, line: number): number {
  const { doc } = view.state;
  const clamped = Math.min(Math.max(0, line), doc.lines - 1);
  const whole = Math.floor(clamped);
  const block = view.lineBlockAt(doc.line(whole + 1).from);
  return block.top + (clamped - whole) * block.height + view.documentPadding.top;
}
