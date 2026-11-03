import { exec, execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, isAbsolute, join, relative, resolve } from 'node:path';
import { promisify } from 'node:util';
import ts from 'typescript';

const git = (...args) => execFileSync('git', args, { encoding: 'utf8', maxBuffer: 1 << 26 });
const lines = (text) => text.split(/\r?\n/).filter(Boolean);
const root = git('rev-parse', '--show-toplevel').trim();
const args = process.argv
  .slice(2)
  .map((file) => relative(root, resolve(file)).replaceAll('\\', '/'))
  .filter((file) => !file.startsWith('../') && !isAbsolute(file));
process.chdir(root);
process.env.PATH = [join(root, 'node_modules', '.bin'), join(homedir(), '.cargo', 'bin'), process.env.PATH].join(delimiter);

const staged = args.length === 0;
const files = staged ? lines(git('diff', '--cached', '--name-only', '--diff-filter=ACMR')) : args.filter((file) => existsSync(file));
const tracked = new Set(staged ? [] : lines(git('ls-files', '--', ...files)));
const added = staged ? lines(git('diff', '--cached', '--name-only', '--diff-filter=A')) : files.filter((file) => !tracked.has(file));
const read = (file) => (staged ? git('show', `:${file}`) : readFileSync(file, 'utf8'));

const K = ts.SyntaxKind;
const ENDS_EXPRESSION = new Set([K.Identifier, K.NumericLiteral, K.BigIntLiteral, K.StringLiteral, K.NoSubstitutionTemplateLiteral, K.TemplateTail, K.RegularExpressionLiteral, K.CloseParenToken, K.CloseBracketToken, K.CloseBraceToken, K.PlusPlusToken, K.MinusMinusToken, K.ThisKeyword, K.SuperKeyword, K.TrueKeyword, K.FalseKeyword, K.NullKeyword]);

const scriptComments = (text, offset = 0) => {
  const scanner = ts.createScanner(ts.ScriptTarget.Latest, false, ts.LanguageVariant.Standard, text);
  const found = [];
  const templates = [];
  let depth = 0;
  let previous;
  for (let kind = scanner.scan(); kind !== K.EndOfFileToken; kind = scanner.scan()) {
    if (kind === K.SingleLineCommentTrivia || kind === K.MultiLineCommentTrivia) found.push([offset + scanner.getTokenStart(), scanner.getTokenText()]);
    if (kind >= K.FirstTriviaToken && kind <= K.LastTriviaToken) continue;
    if (kind === K.OpenBraceToken) depth++;
    if (kind === K.CloseBraceToken && templates.at(-1) === depth) {
      templates.pop();
      kind = scanner.reScanTemplateToken(false);
    } else if (kind === K.CloseBraceToken) depth--;
    if ((kind === K.SlashToken || kind === K.SlashEqualsToken) && !ENDS_EXPRESSION.has(previous)) kind = scanner.reScanSlashToken();
    if (kind === K.TemplateHead || kind === K.TemplateMiddle) templates.push(depth);
    previous = kind;
  }
  return found;
};

const CSS_COMMENT = /"(?:\\[\s\S]|[^"\\])*"|'(?:\\[\s\S]|[^'\\])*'|(?<c>\/\*[\s\S]*?\*\/)/g;
const RUST_COMMENT = /r(#*)"[\s\S]*?"\1|"(?:\\[\s\S]|[^"\\])*"|'(?:\\.[^']*|[^\\'])'|(?<c>\/\/[^\n]*|\/\*[\s\S]*?\*\/)/g;
const HTML_COMMENT = /(?<c><!--[\s\S]*?-->)/g;
const BLOCK = /(<(script|style)\b[^>]*>)([\s\S]*?)<\/\2>/g;

const matches = (pattern, text, offset = 0) => [...text.matchAll(pattern)].filter((m) => m.groups.c).map((m) => [offset + m.index, m[0]]);
const blocks = (text, tag) => [...text.matchAll(BLOCK)].filter((m) => m[2] === tag).map((m) => [m.index + m[1].length, m[3]]);

const commentsOf = (file, text) => {
  if (/\.[cm]?[jt]s$/.test(file)) return scriptComments(text);
  if (file.endsWith('.rs')) return matches(RUST_COMMENT, text);
  if (file.endsWith('.css')) return matches(CSS_COMMENT, text);
  if (!/\.(vue|html)$/.test(file)) return [];
  return [
    ...matches(HTML_COMMENT, text.replace(BLOCK, (block) => ' '.repeat(block.length))),
    ...blocks(text, 'script').flatMap(([offset, body]) => scriptComments(body, offset)),
    ...blocks(text, 'style').flatMap(([offset, body]) => matches(CSS_COMMENT, body, offset)),
  ];
};

const NAMED = 'aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen';
const COLOR = new RegExp(String.raw`#[0-9a-f]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(|(?<![\w.#-])(?:${NAMED.replaceAll(' ', '|')})(?![\w-])`, 'i');
const DECLARATION = /[\w-]+\s*:([^;{}]+)/g;

const rawColors = (file, text) => {
  const css = file.endsWith('.css') ? [[0, text]] : /\.(vue|html)$/.test(file) ? blocks(text, 'style') : [];
  return css.flatMap(([offset, body]) => [...body.matchAll(DECLARATION)].filter((m) => COLOR.test(m[1])).map((m) => [offset + m.index, m[0].trim()]));
};

const lineOf = (text, offset) => text.slice(0, offset).split('\n').length;
const scan = (filter, find) =>
  files.filter(filter).flatMap((file) => {
    const text = read(file);
    return find(file, text).map(([offset, found]) => `${file}:${lineOf(text, offset)}: ${lines(found)[0].trim().slice(0, 100)}`);
  });

const sh = promisify(exec);
const run = async (command) => {
  try {
    await sh(command, { maxBuffer: 1 << 26 });
    return [];
  } catch (error) {
    return lines(`${error.stdout ?? ''}${error.stderr ?? ''}`).slice(-40);
  }
};
const quoted = (list) => list.map((file) => `"${file}"`).join(' ');
const lintable = files.filter((file) => /\.(vue|[cm]?[jt]s)$/.test(file));
const rust = files.filter((file) => file.endsWith('.rs'));
const cargoInput = files.filter((file) => /\.rs$|(^|\/)Cargo\.(toml|lock)$|(^|\/)rust-toolchain\.toml$/.test(file));
const workspaceTouched = cargoInput.some((file) => !file.includes('/'));
const tauriTouched = cargoInput.some((file) => file === 'rust-toolchain.toml' || file.startsWith('apps/desktop/src-tauri/'));
const packageOf = (dir) => readFileSync(`crates/${dir}/Cargo.toml`, 'utf8').match(/^name\s*=\s*"([^"]+)"/m)[1];
const crateDirs = [...new Set(cargoInput.map((file) => file.match(/^crates\/([^/]+)\//)?.[1]).filter((dir) => dir && existsSync(`crates/${dir}/Cargo.toml`)))];
const crateClippy = workspaceTouched ? '--workspace' : crateDirs.map((dir) => `-p ${packageOf(dir)}`).join(' ');

const checks = [
  ['comments', 'удали комментарий: смысл выражай именем, типом или правилом линтера; разрешён только /// <reference>', async () => scan(() => true, (file, text) => commentsOf(file, text).filter(([, raw]) => !/^\/\/\/\s*<reference\s/.test(raw)))],
  ['raw-colors', 'цвет бери из токенов packages/ui-tokens через var(--…); сырые цвета живут только там', async () => scan((file) => /^(apps\/web|packages\/ui-kit)\//.test(file), rawColors)],
  ['new-files', 'не добавляй .md, *.test.*, *.spec.* и __tests__/: правда живёт в коде, тестов в проекте нет', async () => added.filter((file) => /\.md$|\.(test|spec)\.|(^|\/)__tests__\//i.test(file))],
  ['oxlint', 'исправь предупреждения oxlint, не отключай правило', async () => (lintable.length ? run(`oxlint --deny-warnings ${quoted(lintable)}`) : [])],
  ['knip', 'удали неиспользуемый export, файл или зависимость', () => run('knip')],
  ['types', 'исправь ошибки типов', async () => [...(await run('pnpm -r --filter "...[HEAD]" --filter "!veyra" --if-present typecheck')), ...(files.some((file) => file.startsWith('scripts/')) ? await run('tsc -p .') : [])]],
  ['rustfmt', 'отформатируй: cargo fmt --all или cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml', async () => (await Promise.all(Array.from({ length: Math.ceil(rust.length / 50) }, (_, i) => run(`rustfmt --check --edition 2021 ${quoted(rust.slice(i * 50, i * 50 + 50))}`)))).flat()],
  ['clippy', 'исправь замечания clippy, не глуши их через allow', async () => [...(tauriTouched ? await run('cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml -- -D warnings') : []), ...(crateClippy ? await run(`cargo clippy ${crateClippy} -- -D warnings`) : [])]],
];

const results = await Promise.all(checks.map(async ([name, fix, check]) => [name, fix, await check()]));
for (const [name, fix, found] of results) {
  console.log(found.length ? `✗ ${name}: ${found.length} — ${fix}` : `✓ ${name}`);
  for (const line of found) console.log(`    ${line}`);
}
const failed = results.filter(([, , found]) => found.length).map(([name]) => name);
if (failed.length) {
  console.log(`\nпроверки не прошли: ${failed.join(', ')}. Исправь и повтори коммит.`);
  process.exit(1);
}
