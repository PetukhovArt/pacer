#!/usr/bin/env node
// Launcher for the `pacer` npm package.
//
// The binary itself ships in one of the per-platform packages listed as
// optionalDependencies (`@scope/pacer-darwin-arm64`, …); npm installs only
// the one matching this machine's `os`/`cpu`. This file finds it and hands
// over.
//
// `spawnSync` with inherited stdio, not `spawn`: pacer is a full-screen TUI
// that wants the real terminal (raw mode, mouse reporting, resize), and the
// synchronous call also means node is parked inside a syscall for the whole
// session — it cannot run its own SIGINT handler and exit out from under the
// child. Ctrl+C reaches the foreground process group as usual.
'use strict';

const { spawnSync } = require('node:child_process');

// This file is published as `<package>/bin/pacer.js`, so the manifest is one
// level up. The platform package is picked out of our own optionalDependencies
// by suffix, which keeps the launcher working whatever the packages end up
// being called — scoped or not.
const manifest = require('../package.json');
const suffix = `-${process.platform}-${process.arch}`;
const exe = process.platform === 'win32' ? 'pacer.exe' : 'pacer';
const pkg = Object.keys(manifest.optionalDependencies || {}).find((name) =>
  name.endsWith(suffix),
);

let binary;
try {
  if (!pkg) throw new Error(`no package for ${suffix.slice(1)}`);
  binary = require.resolve(`${pkg}/bin/${exe}`);
} catch {
  console.error(
    `pacer: no binary for ${process.platform}-${process.arch}.\n` +
      `\n` +
      `Expected the optional dependency ${pkg || `*${suffix}`}. If you installed\n` +
      `with --no-optional or --omit=optional, reinstall without it. If this\n` +
      `platform has no published build, install from source instead:\n` +
      `\n` +
      `  cargo install --git https://github.com/PetukhovArt/pacer pacer --locked\n`,
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });

if (result.error) {
  console.error(`pacer: failed to run ${binary}: ${result.error.message}`);
  process.exit(1);
}

// A child killed by a signal has no exit status. Re-raise it on ourselves so
// the shell sees the same cause of death it would from the bare binary.
if (result.signal) {
  process.kill(process.pid, result.signal);
}

process.exit(result.status === null ? 1 : result.status);
