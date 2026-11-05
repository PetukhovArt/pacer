import DOMPurify from 'dompurify';
import { marked } from 'marked';
import { highlightCode, languageOf } from './highlight';

export type Rendered = { html: string; diagrams: string[] };

const DIAGRAM_START = new RegExp(
  String.raw`^\s*(?:(?:---[\s\S]*?---|%%[^\n]*)\s*)*(?:(?:graph|flowchart)(?:[ \t]+(?:TB|TD|BT|RL|LR))?[ \t]*(?:;|\n|$)|(?:sequenceDiagram|classDiagram|stateDiagram|erDiagram|gitGraph|gantt)(?:-v2)?(?=\s|$)|(?:journey|mindmap|timeline)[ \t]*(?:\n|$)|pie[ \t]*(?:showData|title\b|\n|$))`,
);

const LANGUAGE_HINT = /(?<!\w)(?:language-|lang-|code-|brush:\s*|syntax-)([\w#+.-]+)/i;
const BARE_ATTRIBUTES = ['data-language', 'data-lang', 'data-syntax', 'lang'];
const HINT_ATTRIBUTES = ['class', 'data-syntaxhighlighter-params'];

function declaredLanguage(el: Element | null): string | null {
  if (!el) return null;
  for (const name of BARE_ATTRIBUTES) {
    const value = el.getAttribute(name)?.trim().toLowerCase();
    if (value) return value;
  }
  const hints = HINT_ATTRIBUTES.map((name) => el.getAttribute(name) ?? '').join(' ');
  if (/\bmermaid\b/i.test(hints)) return 'mermaid';
  return LANGUAGE_HINT.exec(hints)?.[1]?.toLowerCase() ?? null;
}

function slug(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, '')
    .replace(/\s+/g, '-');
}

function toBody(html: string): HTMLElement {
  return DOMPurify.sanitize(html, {
    RETURN_DOM: true,
    FORBID_TAGS: ['style', 'form', 'iframe', 'object', 'embed', 'base', 'meta', 'link'],
  }) as HTMLElement;
}

function markDiagram(target: Element, source: string, diagrams: string[]): void {
  const holder = target.ownerDocument.createElement('div');
  holder.className = 'rc-diagram';
  holder.setAttribute('data-rc-mermaid', String(diagrams.length));
  diagrams.push(source);
  const panel = target.closest('div.code.panel');
  const scope = panel && panel.querySelectorAll('pre').length === 1 ? panel : target;
  scope.replaceWith(holder);
}

function processCode(body: HTMLElement, diagrams: string[]): void {
  for (const pre of body.querySelectorAll('pre')) {
    const code = pre.querySelector('code');
    const declared = declaredLanguage(pre) ?? declaredLanguage(code);
    const text = (code ?? pre).textContent.replace(/\n$/, '');
    if (declared === 'mermaid' || (!declared && DIAGRAM_START.test(text))) {
      markDiagram(pre, text, diagrams);
      continue;
    }
    pre.classList.add('rc-code');
    const language = declared ? languageOf(declared) : null;
    if (declared) pre.setAttribute('data-lang', declared);
    if (language) (code ?? pre).innerHTML = highlightCode(text, language);
    (code ?? pre).classList.add('hljs');
  }
  for (const block of body.querySelectorAll('div.mermaid')) markDiagram(block, block.textContent.trim(), diagrams);
}

function processStructure(body: HTMLElement): void {
  for (const table of body.querySelectorAll('table')) {
    const wrap = table.ownerDocument.createElement('div');
    wrap.className = 'rc-table';
    table.replaceWith(wrap);
    wrap.append(table);
  }
  for (const box of body.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')) {
    box.setAttribute('disabled', '');
    box.closest('li')?.classList.add('rc-task');
    box.closest('ul, ol')?.classList.add('rc-tasks');
  }
  for (const link of body.querySelectorAll('a[href]')) {
    link.removeAttribute('target');
    link.setAttribute('rel', 'noopener noreferrer');
  }
  const used = new Set<string>();
  for (const heading of body.querySelectorAll('h1, h2, h3, h4, h5, h6')) {
    if (heading.id) continue;
    const base = slug(heading.textContent);
    if (!base) continue;
    let id = base;
    for (let n = 1; used.has(id); n++) id = `${base}-${n}`;
    used.add(id);
    heading.id = id;
  }
}

export function renderRich(input: { markdown?: string; html?: string }): Rendered {
  const source = input.markdown !== undefined ? (marked.parse(input.markdown, { async: false, gfm: true, breaks: true }) as string) : (input.html ?? '');
  const body = toBody(source);
  const diagrams: string[] = [];
  processCode(body, diagrams);
  processStructure(body);
  return { html: body.innerHTML, diagrams };
}
