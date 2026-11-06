import { zoomTerminal } from '../agents/terminalFont';
import { isDesktop } from '../bridge';
import { openCleanup } from '../cleanup/state';
import { navigate, openSettings } from '../nav';
import { toggleTheme } from '../settings/appearance';
import { requestSidebarSearch, toggleSidebar } from '../sidebarState';
import { openUpdate } from '../update/state';
import { toggleQuickOpen } from '../workbench/dock';
import { paletteOpen } from './context';
import { openKeymapExternal } from './keymap';
import { defineAction } from './registry';

const NAVIGATION = 'commands.category.navigation';
const VIEW = 'commands.category.view';
const TERMINAL = 'commands.category.terminal';
const KEYS = 'commands.category.keys';
const APP = 'commands.category.app';

defineAction({ id: 'palette.open', title: 'commands.action.paletteOpen', category: VIEW, run: () => (paletteOpen.value = !paletteOpen.value) });

defineAction({ id: 'nav.agents', title: 'commands.action.navAgents', category: NAVIGATION, run: () => navigate({ kind: 'agents' }) });
defineAction({ id: 'nav.reviews', title: 'commands.action.navReviews', category: NAVIGATION, run: () => navigate({ kind: 'reviews' }) });
defineAction({ id: 'nav.jira', title: 'commands.action.navJira', category: NAVIGATION, run: () => navigate({ kind: 'jira' }) });
defineAction({ id: 'nav.confluence', title: 'commands.action.navConfluence', category: NAVIGATION, run: () => navigate({ kind: 'confluence' }) });
defineAction({ id: 'nav.settings', title: 'commands.action.navSettings', category: VIEW, run: () => openSettings() });

defineAction({ id: 'sidebar.toggle', title: 'commands.action.sidebarToggle', category: VIEW, run: toggleSidebar });
defineAction({ id: 'sidebar.search', title: 'commands.action.sidebarSearch', category: NAVIGATION, run: requestSidebarSearch });
defineAction({ id: 'theme.toggle', title: 'commands.action.themeToggle', category: VIEW, run: toggleTheme });

defineAction({ id: 'terminal.zoomIn', title: 'commands.action.terminalZoomIn', category: TERMINAL, run: () => zoomTerminal(1) });
defineAction({ id: 'terminal.zoomOut', title: 'commands.action.terminalZoomOut', category: TERMINAL, run: () => zoomTerminal(-1) });
defineAction({ id: 'terminal.zoomReset', title: 'commands.action.terminalZoomReset', category: TERMINAL, run: () => zoomTerminal(0) });

defineAction({ id: 'agents.quickOpen', title: 'commands.action.agentsQuickOpen', category: NAVIGATION, run: toggleQuickOpen });

defineAction({ id: 'keymap.openSettings', title: 'commands.action.keymapOpenSettings', category: KEYS, run: () => openSettings('keymap') });
if (isDesktop) {
  defineAction({ id: 'app.update', title: 'update.action.update', category: APP, run: () => openUpdate('update') });
  defineAction({ id: 'app.rollback', title: 'update.action.rollback', category: APP, run: () => openUpdate('rollback') });
  defineAction({ id: 'worktrees.cleanup', title: 'cleanup.action', category: APP, run: openCleanup });
}

defineAction({ id: 'keymap.openFile', title: 'commands.action.keymapOpenFile', category: KEYS, run: () => openKeymapExternal() });
