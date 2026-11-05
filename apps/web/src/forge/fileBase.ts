const MARKDOWN_PATH = /\.(?:md|markdown)$/i;

export const isMarkdownPath = (path: string) => MARKDOWN_PATH.test(path);

function encodePath(path: string): string {
  return path.split('/').map(encodeURIComponent).join('/');
}

export function fileBaseUrl(requestUrl: string, sha: string, path: string): string | undefined {
  const dir = path.includes('/') ? path.slice(0, path.lastIndexOf('/') + 1) : '';
  const gitlab = /^(.*?)\/(?:-\/)?merge_requests\/\d+/.exec(requestUrl);
  if (gitlab) return `${gitlab[1]}/-/blob/${sha}/${encodePath(dir)}`;
  const github = /^(.*?)\/pull\/\d+/.exec(requestUrl);
  if (github) return `${github[1]}/blob/${sha}/${encodePath(dir)}`;
  return undefined;
}
