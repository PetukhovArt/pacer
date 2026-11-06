import { closeChannel, createChannel, invoke, isDesktop } from '../bridge';

export type BinaryFile = { mime: string; base64: string };

export type FileDoc = { text: string; signature: string };

export const CHANGED_ON_DISK = 'changed-on-disk';

export const fsReadText = (path: string) => invoke<string>('fs_read_text', { path });
export const fsReadDoc = (path: string) => invoke<FileDoc>('fs_read_doc', { path });
export const fsWriteText = (path: string, text: string, expected: string) => invoke<string>('fs_write_text', { path, text, expected });
export const fsReadBinary = (path: string) => invoke<BinaryFile>('fs_read_binary', { path });
export const fsListMarkdown = (root: string) => invoke<string[]>('fs_list_markdown', { root });
export const fsListDirs = (root: string) => invoke<string[]>('fs_list_dirs', { root });
export const canOpenExternal = isDesktop;
export const fsOpenExternal = (path: string) => invoke<void>('fs_open_external', { path });

export async function fsWatch(path: string, onChange: (doc: FileDoc) => void): Promise<() => void> {
  const channel = createChannel<FileDoc>(onChange);
  const id = await invoke<number>('fs_watch', { path, onEvent: channel });
  return () => {
    closeChannel(channel);
    void invoke<void>('fs_unwatch', { id }).catch(() => undefined);
  };
}
