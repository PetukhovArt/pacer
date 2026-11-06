import { watch } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { useExternalSessions } from './agents/external';
import { useBerth } from './agents/berth';
import { invoke, isDesktop } from './bridge';
import { openUpdate } from './update/state';
import { useDock } from './workbench/dock';

const NEW_SESSION_EVENT = 'tray://new-session';
const UPDATE_EVENT = 'tray://update';
const ROLLBACK_EVENT = 'tray://rollback';

export function installTray(): void {
  if (!isDesktop) return;
  const berth = useBerth();
  const external = useExternalSessions();
  const dock = useDock();
  if (berth) {
    watch(
      () => {
        let running = 0;
        let waiting = 0;
        for (const session of berth.sessions.value.values()) {
          if (session.status === 'running') running += 1;
          else if (session.status === 'needs_feedback') waiting += 1;
        }
        for (const session of [...(external?.sessions.value ?? []), ...(external?.linked.value.values() ?? [])]) {
          if (!session.running) continue;
          if (session.status === 'working') running += 1;
          else if (session.status === 'waiting') waiting += 1;
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
  void listen(UPDATE_EVENT, () => openUpdate('update'));
  void listen(ROLLBACK_EVENT, () => openUpdate('rollback'));
}
