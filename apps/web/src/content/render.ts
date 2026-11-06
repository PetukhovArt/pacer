import DOMPurify from 'dompurify';
import { Marked, Renderer, type MarkedExtension, type TokenizerAndRendererExtension } from 'marked';
import markedFootnote from 'marked-footnote';
import { emojiFor, emojiLoaded } from './emoji';
import { escapeHtml, highlightCode, languageOf } from './highlight';
import { t } from '../i18n';
import { ALERTS, copyButton, type AlertKind } from './icons';

export type Rendered = { html: string; diagrams: string[] };
export type RenderInput = { markdown?: string; html?: string; breaks?: boolean };

type LineNode = { type: string; raw: string; line?: number; tokens?: LineNode[]; items?: LineNode[] };
type Stamped = { line?: number };
type BlockRender = (this: Renderer, token: Stamped) => string;

const DIAGRAM_START = new RegExp(
  String.raw`^\s*(?:(?:---[\s\S]*?---|%%[^\n]*)\s*)*(?:(?:graph|flowchart)(?:[ \t]+(?:TB|TD|BT|RL|LR))?[ \t]*(?:;|\n|$)|(?:sequenceDiagram|classDiagram|stateDiagram|erDiagram|gitGraph|gantt)(?:-v2)?(?=\s|$)|(?:journey|mindmap|timeline)[ \t]*(?:\n|$)|pie[ \t]*(?:showData|title\b|\n|$))`,
);

const LANGUAGE_HINT = /(?<!\w)(?:language-|lang-|code-|brush:\s*|syntax-)([\w#+.-]+)/i;
const BARE_ATTRIBUTES = ['data-language', 'data-lang', 'data-syntax', 'lang'];
const HINT_ATTRIBUTES = ['class', 'data-syntaxhighlighter-params'];
const FRONT_MATTER = /^﻿?---[ \t]*\r?\n([\s\S]*?)\r?\n(?:---|\.\.\.)[ \t]*(?:\r?\n|$)/;
const ALERT_MARK = /^\s*\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\][ \t]*/i;
const SHORTCODE = /:([a-z0-9_+-]+):/g;
const HAS_SHORTCODE = /:[a-z0-9_+-]+:/;
const STAMPED_BLOCKS = ['heading', 'paragraph', 'code', 'blockquote', 'list', 'table', 'hr'] as const;

let lineOffset = 0;
let lineSource = '';

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

function newlines(text: string): number {
  let count = 0;
  for (let at = text.indexOf('\n'); at !== -1; at = text.indexOf('\n', at + 1)) count++;
  return count;
}

function annotate(tokens: LineNode[], base: number, source?: string): void {
  let line = base;
  let pos = 0;
  let seen = 0;
  for (const token of tokens) {
    if (source !== undefined) {
      if (token.type === 'footnotes') continue;
      const at = source.indexOf(token.raw, pos);
      if (at >= 0) {
        line += newlines(source.slice(seen, at));
        seen = at;
        pos = at + token.raw.length;
      }
    }
    token.line = line;
    if (token.type === 'blockquote' && token.tokens) annotate(token.tokens, line);
    else if (token.type === 'list' && token.items) {
      let at = line;
      for (const item of token.items) {
        item.line = at;
        if (item.tokens) annotate(item.tokens, at);
        at += newlines(item.raw);
      }
    }
    if (source === undefined) line += newlines(token.raw);
  }
}

function stamp(html: string, token: Stamped): string {
  return token.line === undefined ? html : html.replace(/^(\s*<[a-z][a-z0-9]*)/, `$1 data-line="${token.line}"`);
}

function stampedRenderer(): MarkedExtension['renderer'] {
  const base = Renderer.prototype as unknown as Record<string, BlockRender>;
  const methods: Record<string, BlockRender> = {};
  for (const type of STAMPED_BLOCKS) {
    const render = base[type] as BlockRender;
    methods[type] = function (this: Renderer, token) {
      return stamp(render.call(this, token), token);
    };
  }
  return methods as unknown as MarkedExtension['renderer'];
}

const mathBlock: TokenizerAndRendererExtension = {
  name: 'mathBlock',
  level: 'block',
  start: (src) => src.match(/^\$\$/m)?.index,
  tokenizer(src) {
    const match = /^\$\$[ \t]*\n?([\s\S]+?)\n?[ \t]*\$\$[ \t]*(?:\n|$)/.exec(src);
    if (!match) return undefined;
    return { type: 'mathBlock', raw: match[0], text: (match[1] ?? '').trim() };
  },
  renderer(token) {
    return stamp(`<div class="rc-math" data-display="block">${escapeHtml(String(token.text))}</div>\n`, token as Stamped);
  },
};

const mathInline: TokenizerAndRendererExtension = {
  name: 'mathInline',
  level: 'inline',
  start: (src) => {
    const at = src.indexOf('$');
    return at < 0 ? undefined : at;
  },
  tokenizer(src) {
    const display = /^\$\$([^\n]+?)\$\$/.exec(src);
    if (display) return { type: 'mathInline', raw: display[0], text: (display[1] ?? '').trim(), display: true };
    const inline = /^\$(?![\s$])((?:\\.|[^$\\\n])+?)(?<![\s\\])\$(?!\d)/.exec(src);
    if (inline) return { type: 'mathInline', raw: inline[0], text: inline[1] ?? '', display: false };
    return undefined;
  },
  renderer(token) {
    return `<span class="rc-math" data-display="${token.display ? 'block' : 'inline'}">${escapeHtml(String(token.text))}</span>`;
  },
};

function engine(breaks: boolean): Marked {
  return new Marked({ gfm: true, breaks }).use(markedFootnote()).use({
    extensions: [mathBlock, mathInline],
    renderer: stampedRenderer(),
    hooks: {
      processAllTokens(tokens) {
        annotate(tokens as unknown as LineNode[], lineOffset, lineSource);
        return tokens;
      },
    },
  });
}

function toBody(html: string): HTMLElement {
  return DOMPurify.sanitize(html, {
    RETURN_DOM: true,
    FORBID_TAGS: ['style', 'form', 'iframe', 'object', 'embed', 'base', 'meta', 'link'],
  }) as HTMLElement;
}

function unquote(value: string): string {
  const text = value.trim();
  const quoted = /^(["'])(.*)\1$/.exec(text);
  return (quoted ? quoted[2] : text.replace(/^\[(.*)\]$/, '$1')) ?? '';
}

function parseFrontMatter(block: string): [string, string][] {
  const entries: [string, string[]][] = [];
  let current: [string, string[]] | null = null;
  for (const line of block.split(/\r?\n/)) {
    const key = /^([A-Za-z0-9_.-]+):[ \t]*(.*)$/.exec(line);
    if (key) {
      current = [key[1] ?? '', key[2] ? [unquote(key[2])] : []];
      entries.push(current);
      continue;
    }
    if (!current || !line.trim()) continue;
    const item = /^\s*-\s+(.*)$/.exec(line);
    current[1].push(unquote(item ? (item[1] ?? '') : line));
  }
  return entries.map(([key, values]) => [key, values.join(', ')]);
}

function frontMatterHtml(entries: [string, string][]): string {
  const rows = entries.map(([key, value]) => `<tr><th>${escapeHtml(key)}</th><td>${escapeHtml(value)}</td></tr>`).join('');
  return `<div class="rc-frontmatter" data-line="0"><table><tbody>${rows}</tbody></table></div>`;
}

function markDiagram(target: Element, source: string, diagrams: string[]): void {
  const holder = target.ownerDocument.createElement('div');
  holder.className = 'rc-diagram';
  holder.setAttribute('data-rc-mermaid', String(diagrams.length));
  const line = target.getAttribute('data-line');
  if (line) holder.setAttribute('data-line', line);
  diagrams.push(source);
  const panel = target.closest('div.code.panel');
  const scope = panel && panel.querySelectorAll('pre').length === 1 ? panel : target;
  scope.replaceWith(holder);
}

function markMath(pre: Element, source: string): void {
  const holder = pre.ownerDocument.createElement('div');
  holder.className = 'rc-math';
  holder.setAttribute('data-display', 'block');
  const line = pre.getAttribute('data-line');
  if (line) holder.setAttribute('data-line', line);
  holder.textContent = source;
  pre.replaceWith(holder);
}

function wrapCode(pre: Element, label: string | null): void {
  const doc = pre.ownerDocument;
  const box = doc.createElement('div');
  box.className = 'rc-codeblock';
  const line = pre.getAttribute('data-line');
  if (line) {
    box.setAttribute('data-line', line);
    pre.removeAttribute('data-line');
  }
  pre.replaceWith(box);
  box.append(pre);
  if (label) {
    const tag = doc.createElement('span');
    tag.className = 'rc-lang';
    tag.textContent = label;
    box.append(tag);
  }
  box.insertAdjacentHTML('beforeend', copyButton());
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
    if (declared === 'math') {
      markMath(pre, text);
      continue;
    }
    pre.classList.add('rc-code');
    const language = declared ? languageOf(declared) : null;
    if (declared) pre.setAttribute('data-lang', declared);
    if (language) (code ?? pre).innerHTML = highlightCode(text, language);
    (code ?? pre).classList.add('hljs');
    wrapCode(pre, declared);
  }
  for (const block of body.querySelectorAll('div.mermaid')) markDiagram(block, block.textContent.trim(), diagrams);
}

function processAlerts(body: HTMLElement): void {
  for (const quote of body.querySelectorAll('blockquote')) {
    const first = quote.firstElementChild;
    const lead = first?.firstChild;
    if (first?.tagName !== 'P' || lead?.nodeType !== Node.TEXT_NODE) continue;
    const match = ALERT_MARK.exec(lead.textContent ?? '');
    if (!match) continue;
    const kind = (match[1] ?? 'note').toLowerCase() as AlertKind;
    lead.textContent = (lead.textContent ?? '').slice(match[0].length);
    if (!lead.textContent) lead.remove();
    if (first.firstChild?.nodeName === 'BR') first.firstChild.remove();
    const doc = quote.ownerDocument;
    const box = doc.createElement('div');
    box.className = 'rc-alert';
    box.setAttribute('data-kind', kind);
    const line = quote.getAttribute('data-line');
    if (line) box.setAttribute('data-line', line);
    const title = doc.createElement('p');
    title.className = 'rc-alert-title';
    title.innerHTML = `${ALERTS[kind].icon}<span>${escapeHtml(t(ALERTS[kind].titleKey))}</span>`;
    box.append(title, ...quote.childNodes);
    if (!first.hasChildNodes()) first.remove();
    quote.replaceWith(box);
  }
}

function processEmoji(body: HTMLElement): void {
  if (!emojiLoaded()) return;
  const walker = body.ownerDocument.createTreeWalker(body, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (node.parentElement?.closest('code, pre, .rc-math')) continue;
    if (HAS_SHORTCODE.test(node.textContent ?? '')) nodes.push(node as Text);
  }
  for (const node of nodes) {
    node.textContent = (node.textContent ?? '').replace(SHORTCODE, (whole, name: string) => emojiFor(name) ?? whole);
  }
}

function processStructure(body: HTMLElement, anchors: boolean): void {
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
    if (!heading.id) {
      const base = slug(heading.textContent);
      if (!base) continue;
      let id = base;
      for (let n = 1; used.has(id); n++) id = `${base}-${n}`;
      used.add(id);
      heading.id = id;
    } else {
      used.add(heading.id);
    }
    if (anchors && !heading.classList.contains('sr-only')) {
      const link = heading.ownerDocument.createElement('a');
      link.className = 'rc-anchor';
      link.setAttribute('href', `#${heading.id}`);
      link.setAttribute('aria-label', t('content.sectionLink'));
      heading.append(link);
    }
  }
}

export function renderRich(input: RenderInput): Rendered {
  const diagrams: string[] = [];
  let prefix = '';
  let source: string;
  if (input.markdown !== undefined) {
    let markdown = input.markdown.replace(/\r\n?/g, '\n');
    lineOffset = 0;
    const front = FRONT_MATTER.exec(markdown);
    if (front) {
      const entries = parseFrontMatter(front[1] ?? '');
      if (entries.length) {
        prefix = frontMatterHtml(entries);
        lineOffset = newlines(front[0]);
        markdown = markdown.slice(front[0].length);
      }
    }
    lineSource = markdown;
    source = engine(input.breaks ?? true).parse(markdown, { async: false }) as string;
  } else {
    source = input.html ?? '';
  }
  const body = toBody(source);
  processCode(body, diagrams);
  processAlerts(body);
  processStructure(body, input.markdown !== undefined);
  if (input.markdown !== undefined) processEmoji(body);
  return { html: prefix + body.innerHTML, diagrams };
}
