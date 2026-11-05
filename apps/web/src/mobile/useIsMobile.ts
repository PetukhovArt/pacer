import { readonly, shallowRef, type Ref } from 'vue';

const QUERY = '(max-width: 699px)';

let state: Ref<boolean> | null = null;

export function useIsMobile(): Readonly<Ref<boolean>> {
  if (!state) {
    const media = window.matchMedia(QUERY);
    const mobile = shallowRef(media.matches);
    media.addEventListener('change', (event) => {
      mobile.value = event.matches;
    });
    state = mobile;
  }
  return readonly(state);
}
