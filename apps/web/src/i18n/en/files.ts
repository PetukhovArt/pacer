export const files = {
  markdown: {
    viewMode: 'View mode',
    code: 'Code',
    split: 'Split',
    preview: 'Preview',
    toc: 'Contents',
    openExternalTitle: 'Open in the default application',
    openExternal: 'Open in editor',
  },
  picker: {
    open: 'Open a Markdown file',
    filter: 'Filter Markdown files',
    none: 'No Markdown files in this worktree',
  },
  switcher: {
    dialog: 'Quick jump to a session or file',
    search: 'Search sessions and files',
    placeholderFiles: 'workspace › project › branch › session, or a Markdown file in {scope}',
  },
} as const;
