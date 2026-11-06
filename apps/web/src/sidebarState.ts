import { ref, watch } from 'vue';

const COLLAPSED_KEY = 'berth.sidebar.collapsed';

export const sidebarCollapsed = ref(localStorage.getItem(COLLAPSED_KEY) === '1');
export const sidebarSearchRequests = ref(0);

watch(sidebarCollapsed, (value) => localStorage.setItem(COLLAPSED_KEY, value ? '1' : '0'));

export function toggleSidebar(): void {
  sidebarCollapsed.value = !sidebarCollapsed.value;
}

export function requestSidebarSearch(): void {
  sidebarSearchRequests.value++;
}
