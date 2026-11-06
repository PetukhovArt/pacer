import { cssToken } from './theme';

let loading: Promise<typeof import('katex').default> | null = null;

function load() {
  loading ??= Promise.all([import('katex'), import('katex/dist/katex.min.css')]).then(([module]) => module.default);
  return loading;
}

export async function renderMath(root: HTMLElement, isCurrent: () => boolean): Promise<void> {
  if (!root.querySelector('.rc-math:not([data-rendered])')) return;
  let katex: Awaited<ReturnType<typeof load>>;
  try {
    katex = await load();
  } catch {
    loading = null;
    return;
  }
  if (!isCurrent()) return;
  const errorColor = cssToken('color-danger') || 'currentColor';
  for (const node of root.querySelectorAll<HTMLElement>('.rc-math:not([data-rendered])')) {
    const source = node.textContent;
    node.setAttribute('data-rendered', '');
    node.setAttribute('data-tex', source);
    katex.render(source, node, { displayMode: node.dataset.display === 'block', throwOnError: false, errorColor });
  }
}
