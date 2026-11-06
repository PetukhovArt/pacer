export type KeymapEntry = { context?: string; bindings: Record<string, string | null> };

export const DEFAULT_KEYMAP: readonly KeymapEntry[] = [
  {
    bindings: {
      'ctrl-shift-p': 'palette.open',
      f1: 'palette.open',
      'ctrl-,': 'nav.settings',
      'ctrl-alt-p': 'agents.quickOpen',
    },
  },
  {
    context: 'terminalFocus',
    bindings: {
      'ctrl-=': 'terminal.zoomIn',
      'ctrl--': 'terminal.zoomOut',
      'ctrl-0': 'terminal.zoomReset',
    },
  },
  {
    context: '!terminalFocus && !editorFocus',
    bindings: {
      'ctrl-b': 'sidebar.toggle',
      'ctrl-k': 'sidebar.search',
    },
  },
  {
    context: '!terminalFocus',
    bindings: {
      'ctrl-p': 'agents.quickOpen',
      'ctrl-shift-r': 'nav.reviews',
      'ctrl-shift-j': 'nav.jira',
    },
  },
];
