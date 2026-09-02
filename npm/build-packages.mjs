// Lay out the npm packages for a release: one per platform, each carrying a
// single binary, plus the launcher package that depends on all of them and
// picks the right one at runtime.
//
// This is the esbuild/biome distribution shape. The alternative — one package
// with a postinstall that downloads a binary — needs network at install time,
// breaks behind proxies and offline caches, and runs code on install. Here npm
// resolves `os`/`cpu` itself and fetches exactly one platform package.
//
//   node npm/build-packages.mjs --version 0.17.0 --bins <dir> [--scope @petukhovart]
//
// <dir> holds one subdirectory per rust target, each with the built binary:
//   <dir>/x86_64-unknown-linux-musl/pacer
//   <dir>/x86_64-pc-windows-msvc/pacer.exe
//
// Output lands in npm/dist/, ready for `npm publish` — platform packages
// first, launcher last, so an install can never resolve a launcher whose
// dependencies do not exist yet.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(HERE, '..');
const DIST = path.join(HERE, 'dist');

const REPO = 'https://github.com/PetukhovArt/pacer';

/** Rust target → the npm `os`/`cpu` pair npm matches against. */
const TARGETS = [
  { target: 'aarch64-apple-darwin', os: 'darwin', cpu: 'arm64' },
  { target: 'x86_64-apple-darwin', os: 'darwin', cpu: 'x64' },
  { target: 'x86_64-unknown-linux-musl', os: 'linux', cpu: 'x64' },
  { target: 'aarch64-unknown-linux-musl', os: 'linux', cpu: 'arm64' },
  { target: 'x86_64-pc-windows-msvc', os: 'win32', cpu: 'x64' },
];

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  if (i !== -1 && process.argv[i + 1]) return process.argv[i + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`missing --${name}`);
}

const version = arg('version').replace(/^v/, '');
const bins = path.resolve(arg('bins'));
const scope = arg('scope', process.env.NPM_SCOPE ?? '').replace(/\/$/, '');
/** `@petukhovart/pacer` when scoped, `pacer` otherwise — same launcher either way. */
const name = (suffix) =>
  scope ? `${scope}/pacer${suffix}` : `pacer${suffix ? suffix : ''}`;

const common = {
  version,
  license: 'MIT',
  homepage: `${REPO}#readme`,
  repository: { type: 'git', url: `git+${REPO}.git` },
  bugs: { url: `${REPO}/issues` },
};

fs.rmSync(DIST, { recursive: true, force: true });

const built = [];

for (const { target, os, cpu } of TARGETS) {
  const exe = os === 'win32' ? 'pacer.exe' : 'pacer';
  const source = path.join(bins, target, exe);
  if (!fs.existsSync(source)) {
    throw new Error(`no binary at ${source} — is --bins pointing at the right dir?`);
  }

  const pkg = name(`-${os}-${cpu}`);
  const dir = path.join(DIST, `pacer-${os}-${cpu}`);
  fs.mkdirSync(path.join(dir, 'bin'), { recursive: true });

  const dest = path.join(dir, 'bin', exe);
  fs.copyFileSync(source, dest);
  // GitHub artifact download drops the executable bit; npm packs whatever
  // mode the file has, so an unfixed 0644 here ships a binary nobody can run.
  fs.chmodSync(dest, 0o755);

  write(path.join(dir, 'package.json'), {
    name: pkg,
    description: `pacer binary for ${os}-${cpu}`,
    ...common,
    os: [os],
    cpu: [cpu],
    files: ['bin'],
  });

  built.push({ pkg, dir });
}

// The launcher. `optionalDependencies` are pinned exactly: a launcher must
// never pair with a binary from another version.
const launcherDir = path.join(DIST, 'pacer');
fs.mkdirSync(path.join(launcherDir, 'bin'), { recursive: true });
fs.copyFileSync(
  path.join(HERE, 'launcher', 'pacer.js'),
  path.join(launcherDir, 'bin', 'pacer.js'),
);
fs.copyFileSync(path.join(HERE, 'launcher', 'README.md'), path.join(launcherDir, 'README.md'));
fs.copyFileSync(path.join(ROOT, 'LICENSE'), path.join(launcherDir, 'LICENSE'));

write(path.join(launcherDir, 'package.json'), {
  name: name(''),
  description: 'Mission control for your coding agents — run Claude Code, Codex and Cursor across every project and git worktree from one terminal',
  ...common,
  keywords: ['claude', 'claude-code', 'codex', 'cursor', 'ai', 'agent', 'tui', 'terminal', 'multiplexer', 'worktree'],
  bin: { pacer: 'bin/pacer.js' },
  files: ['bin', 'README.md', 'LICENSE'],
  engines: { node: '>=16' },
  optionalDependencies: Object.fromEntries(built.map(({ pkg }) => [pkg, version])),
});

built.push({ pkg: name(''), dir: launcherDir });

function write(file, json) {
  fs.writeFileSync(file, `${JSON.stringify(json, null, 2)}\n`);
}

// A smoke test worth the two seconds: run the launcher against this machine's
// own platform package, so a broken resolve path is caught here and not by the
// first person to `npm i -g`.
const self = TARGETS.find((t) => t.os === process.platform && t.cpu === process.arch);
if (self) {
  const link = path.join(launcherDir, 'node_modules', name(`-${self.os}-${self.cpu}`));
  fs.mkdirSync(path.dirname(link), { recursive: true });
  fs.cpSync(path.join(DIST, `pacer-${self.os}-${self.cpu}`), link, { recursive: true });
  const out = execFileSync(process.execPath, [path.join(launcherDir, 'bin', 'pacer.js'), '--version'], {
    encoding: 'utf8',
  }).trim();
  fs.rmSync(path.join(launcherDir, 'node_modules'), { recursive: true, force: true });
  if (!out.includes(version)) {
    throw new Error(`launcher smoke test printed "${out}", expected version ${version}`);
  }
  console.log(`launcher smoke test: ${out}`);
}

console.log(`\nbuilt ${built.length} packages in ${path.relative(process.cwd(), DIST)}:`);
for (const { pkg, dir } of built) {
  console.log(`  ${pkg}  (${path.relative(DIST, dir)})`);
}
console.log('\npublish platform packages first, launcher last.');
