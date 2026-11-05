import { closeChannel, createChannel, invoke, isDesktop } from '../bridge';

export type BinaryFile = { mime: string; base64: string };

export const fsReadText = (path: string) => invoke<string>('fs_read_text', { path });
export const fsReadBinary = (path: string) => invoke<BinaryFile>('fs_read_binary', { path });
export const fsListMarkdown = (root: string) => invoke<string[]>('fs_list_markdown', { root });
export const canOpenExternal = isDesktop;
export const fsOpenExternal = (path: string) => invoke<void>('fs_open_external', { path });

export async function fsWatch(path: string, onChange: (text: string) => void): Promise<() => void> {
  const channel = createChannel<{ text: string }>((event) => onChange(event.text));
  const id = await invoke<number>('fs_watch', { path, onEvent: channel });
  return () => {
    closeChannel(channel);
    void invoke<void>('fs_unwatch', { id }).catch(() => undefined);
  };
}
