import { ref, type Ref } from 'vue';

const DARK_CLASS = 'vr-dark';
const dark = ref(false);
let watching = false;

function read(): boolean {
  return document.documentElement.classList.contains(DARK_CLASS);
}

export function useDarkTheme(): Ref<boolean> {
  if (!watching) {
    watching = true;
    dark.value = read();
    new MutationObserver(() => {
      dark.value = read();
    }).observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
  }
  return dark;
}

export function cssToken(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(`--vr-${name}`).trim();
}
