import { nextTick } from 'vue';
import { isMotionEnabled, setMotionEnabled } from '@ui/kit';

export { setMotionEnabled };

export async function withViewTransition(update: () => void | Promise<void>): Promise<void> {
  if (!isMotionEnabled() || typeof document.startViewTransition !== 'function') {
    await update();
    await nextTick();
    return;
  }
  const transition = document.startViewTransition(async () => {
    await update();
    await nextTick();
  });
  transition.ready.catch(() => undefined);
  transition.finished.catch(() => undefined);
  await transition.updateCallbackDone;
}
