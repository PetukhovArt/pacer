import { zoomTerminal } from '../agents/terminalFont';
import { quickOpen } from '../agents/proto/quickOpen';
import { navigate, openSettings } from '../nav';
import { toggleTheme } from '../settings/appearance';
import { requestSidebarSearch, toggleSidebar } from '../sidebarState';
import { paletteOpen } from './context';
import { openKeymapExternal } from './keymap';
import { defineAction } from './registry';

const NAVIGATION = 'commands.category.navigation';
const VIEW = 'commands.category.view';
const TERMINAL = 'commands.category.terminal';
const KEYS = 'commands.category.keys';

defineAction({ id: 'palette.open', title: 'commands.action.paletteOpen', category: VIEW, run: () => (paletteOpen.value = !paletteOpen.value) });

defineAction({ id: 'nav.tasks', title: 'commands.action.navTasks', category: NAVIGATION, run: () => navigate({ kind: 'tasks' }) });
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

defineAction({ id: 'agents.quickOpen', title: 'commands.action.agentsQuickOpen', category: NAVIGATION, when: "view == 'agents'", run: quickOpen });

defineAction({ id: 'keymap.openSettings', title: 'commands.action.keymapOpenSettings', category: KEYS, run: () => openSettings('keymap') });
defineAction({ id: 'keymap.openFile', title: 'commands.action.keymapOpenFile', category: KEYS, run: () => openKeymapExternal() });
