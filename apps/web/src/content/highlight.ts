import hljs from 'highlight.js/lib/core';
import bash from 'highlight.js/lib/languages/bash';
import cpp from 'highlight.js/lib/languages/cpp';
import csharp from 'highlight.js/lib/languages/csharp';
import css from 'highlight.js/lib/languages/css';
import diff from 'highlight.js/lib/languages/diff';
import dockerfile from 'highlight.js/lib/languages/dockerfile';
import go from 'highlight.js/lib/languages/go';
import ini from 'highlight.js/lib/languages/ini';
import java from 'highlight.js/lib/languages/java';
import javascript from 'highlight.js/lib/languages/javascript';
import json from 'highlight.js/lib/languages/json';
import kotlin from 'highlight.js/lib/languages/kotlin';
import markdown from 'highlight.js/lib/languages/markdown';
import powershell from 'highlight.js/lib/languages/powershell';
import python from 'highlight.js/lib/languages/python';
import rust from 'highlight.js/lib/languages/rust';
import scss from 'highlight.js/lib/languages/scss';
import sql from 'highlight.js/lib/languages/sql';
import typescript from 'highlight.js/lib/languages/typescript';
import xml from 'highlight.js/lib/languages/xml';
import yaml from 'highlight.js/lib/languages/yaml';
import './highlight.css';

const GRAMMARS = { bash, cpp, csharp, css, diff, dockerfile, go, ini, java, javascript, json, kotlin, markdown, powershell, python, rust, scss, sql, typescript, xml, yaml };

for (const [name, grammar] of Object.entries(GRAMMARS)) hljs.registerLanguage(name, grammar);

const TOKENS: Record<string, keyof typeof GRAMMARS> = {
  ts: 'typescript', tsx: 'typescript', mts: 'typescript', cts: 'typescript', typescript: 'typescript',
  js: 'javascript', jsx: 'javascript', mjs: 'javascript', cjs: 'javascript', javascript: 'javascript', node: 'javascript',
  vue: 'xml', html: 'xml', htm: 'xml', xhtml: 'xml', xml: 'xml', svg: 'xml', xsl: 'xml', xsd: 'xml', csproj: 'xml', plist: 'xml',
  rs: 'rust', rust: 'rust',
  py: 'python', pyw: 'python', python: 'python',
  go: 'go', golang: 'go',
  java: 'java',
  kt: 'kotlin', kts: 'kotlin', kotlin: 'kotlin',
  cs: 'csharp', csharp: 'csharp', 'c#': 'csharp',
  c: 'cpp', h: 'cpp', cc: 'cpp', cpp: 'cpp', cxx: 'cpp', hpp: 'cpp', hh: 'cpp', hxx: 'cpp', 'c++': 'cpp',
  json: 'json', jsonc: 'json', json5: 'json', geojson: 'json',
  yml: 'yaml', yaml: 'yaml',
  toml: 'ini', ini: 'ini', cfg: 'ini', conf: 'ini', properties: 'ini',
  sql: 'sql',
  sh: 'bash', bash: 'bash', zsh: 'bash', shell: 'bash', console: 'bash',
  ps1: 'powershell', psm1: 'powershell', psd1: 'powershell', powershell: 'powershell', pwsh: 'powershell',
  md: 'markdown', markdown: 'markdown', mdx: 'markdown',
  css: 'css', scss: 'scss', sass: 'scss',
  diff: 'diff', patch: 'diff',
  dockerfile: 'dockerfile',
};

const MAX_HIGHLIGHT_LENGTH = 300_000;
const TAG = /<span[^>]*>|<\/span>|\n/g;

export function escapeHtml(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

export function languageOf(token: string): string | null {
  return TOKENS[token.trim().toLowerCase()] ?? null;
}

function languageOfPath(path: string): string | null {
  const name = path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1).toLowerCase();
  const dot = name.lastIndexOf('.');
  return languageOf(dot > 0 ? name.slice(dot + 1) : name);
}

export function highlightCode(code: string, language: string | null): string {
  if (!language || code.length > MAX_HIGHLIGHT_LENGTH) return escapeHtml(code);
  try {
    return hljs.highlight(code, { language, ignoreIllegals: true }).value;
  } catch {
    return escapeHtml(code);
  }
}

function splitHighlighted(html: string): string[] {
  const lines: string[] = [];
  const open: string[] = [];
  let line = '';
  let last = 0;
  for (const match of html.matchAll(TAG)) {
    line += html.slice(last, match.index);
    last = match.index + match[0].length;
    const tag = match[0];
    if (tag === '\n') {
      lines.push(line + '</span>'.repeat(open.length));
      line = open.join('');
    } else if (tag === '</span>') {
      open.pop();
      line += tag;
    } else {
      open.push(tag);
      line += tag;
    }
  }
  lines.push(line + html.slice(last));
  return lines;
}

export function highlightLines(code: string, path: string): string[] {
  const text = code.replace(/\r\n?/g, '\n');
  const language = languageOfPath(path);
  if (!language) return text.split('\n').map(escapeHtml);
  return splitHighlighted(highlightCode(text, language));
}
