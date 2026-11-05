import { watch } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { usePacer } from './agents/pacer';
import { invoke, isDesktop } from './bridge';
import { useDock } from './workbench/dock';

const NEW_SESSION_EVENT = 'tray://new-session';

export function installTray(): void {
  if (!isDesktop) return;
  const pacer = usePacer();
  const dock = useDock();
  if (pacer) {
    watch(
      () => {
        let running = 0;
        let waiting = 0;
        for (const session of pacer.sessions.value.values()) {
          if (session.status === 'running') running += 1;
          else if (session.status === 'needs_feedback') waiting += 1;
        }
        return `${running}:${waiting}`;
      },
      (counts) => {
        const [running, waiting] = counts.split(':').map(Number);
        void invoke('tray_status', { running, waiting }).catch(() => undefined);
      },
      { immediate: true },
    );
  }
  void listen(NEW_SESSION_EVENT, () => dock.startCreateInMain());
}
