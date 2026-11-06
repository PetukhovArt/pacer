import { t } from '../i18n';

const svg = (body: string, size = 16) =>
  `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;

export type AlertKind = 'note' | 'tip' | 'important' | 'warning' | 'caution';

export const ALERTS = {
  note: {
    titleKey: 'content.alert.note',
    icon: svg('<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>'),
  },
  tip: {
    titleKey: 'content.alert.tip',
    icon: svg('<path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6"/><path d="M10 22h4"/>'),
  },
  important: {
    titleKey: 'content.alert.important',
    icon: svg('<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M12 7v2"/><path d="M12 13h.01"/>'),
  },
  warning: {
    titleKey: 'content.alert.warning',
    icon: svg('<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>'),
  },
  caution: {
    titleKey: 'content.alert.caution',
    icon: svg(
      '<path d="M12 16h.01"/><path d="M12 8v4"/><path d="M15.312 2a2 2 0 0 1 1.414.586l4.688 4.688A2 2 0 0 1 22 8.688v6.624a2 2 0 0 1-.586 1.414l-4.688 4.688a2 2 0 0 1-1.414.586H8.688a2 2 0 0 1-1.414-.586l-4.688-4.688A2 2 0 0 1 2 15.312V8.688a2 2 0 0 1 .586-1.414l4.688-4.688A2 2 0 0 1 8.688 2z"/>',
    ),
  },
} as const satisfies Record<AlertKind, { titleKey: string; icon: string }>;

export const copyButton = () => `<button type="button" class="rc-copy" title="${t('content.copy')}" aria-label="${t('content.copyCode')}"><span class="rc-copy-idle">${svg('<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>', 14)}</span><span class="rc-copy-done">${svg('<path d="M20 6 9 17l-5-5"/>', 14)}</span></button>`;
