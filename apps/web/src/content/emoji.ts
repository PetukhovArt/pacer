const SHORTCODE = /:[a-z0-9_+-]+:/;

let names: Map<string, string> | null = null;
let loading: Promise<void> | null = null;

export function hasShortcode(text: string): boolean {
  return SHORTCODE.test(text);
}

export function emojiLoaded(): boolean {
  return names !== null;
}

export function emojiFor(name: string): string | undefined {
  return names?.get(name);
}

export function loadEmoji(): Promise<void> {
  loading ??= import('gemoji')
    .then(({ gemoji }) => {
      const map = new Map<string, string>();
      for (const entry of gemoji) for (const name of entry.names) map.set(name, entry.emoji);
      names = map;
    })
    .catch(() => {
      loading = null;
    });
  return loading;
}
