# Releasing

Three channels, one set of binaries:

| Channel | Who it serves | Built by |
|---|---|---|
| GitHub release | `install.sh`, `pacer upgrade`, manual downloads | `.github/workflows/release.yml`, on a `v*` tag |
| npm | anyone with Node — and the only frictionless path on Windows | `.github/workflows/npm.yml`, run by hand |
| `cargo install --git` | source builds, fallback when no asset matches | nothing to do |

crates.io is not a channel: the workspace pulls a patched `vt100` through `[patch.crates-io]`, and
`cargo publish` refuses a patched dependency. `cargo install --git` is unaffected — it builds the
workspace as it stands, vendored fork and all.

## Cutting a release

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]`), run `cargo check` so `Cargo.lock`
   follows, and commit.
2. Tag and push:
   ```sh
   git tag v0.18.0 && git push --tags
   ```
3. The `Release` workflow builds five targets — macOS arm64/x64, Linux x64/arm64 (static musl),
   Windows x64 — and attaches `pacer-<target>.tar.gz` (and `.zip` for Windows) to a GitHub release with
   generated notes. A tag whose version disagrees with `Cargo.toml` fails the build before anything is
   published.
4. To check a cross-compile without cutting a tag, run the same workflow from the Actions tab
   (`workflow_dispatch`) — it builds the matrix and skips the release job.

`install.sh` downloads those assets, so the repository has to be public for the one-liner to work. While
it is private, the script falls back to `cargo install --git`, which needs the user's git credentials.

## Publishing to npm

npm hosts the binaries itself, so this works whether or not the repository is public.

The layout is the esbuild/biome one: five platform packages (`pacer-darwin-arm64`, …), each carrying a
single binary and declaring its `os`/`cpu`, plus a launcher package that lists all five as
`optionalDependencies` and execs whichever one npm installed. No postinstall, no download at install
time.

The packages publish as **`@petukhovart/pacer`** plus `@petukhovart/pacer-<os>-<cpu>`. The scope is there
because plain `pacer` is taken on npm — an abandoned Redis rate-limiter, last published 2016. It changes
the install command only; the binary and the command stay `pacer`.

**One-time setup**

Create an npm **automation** access token and add it to the repository as the `NPM_TOKEN` secret.

**Each release**

Run the `Publish to npm` workflow from the Actions tab with the tag (`v0.18.0`); the scope input is
prefilled. It
downloads that release's assets — the same binaries, never a rebuild — lays out the packages, and:

- with **dry run** left on (the default) runs `npm pack --dry-run` over each package so you can read the
  file lists before anything leaves the machine;
- with dry run off, publishes the platform packages first and the launcher last, so the registry never
  holds a launcher whose dependencies do not resolve yet.

The publish step is manual on purpose: `npm unpublish` is a 72-hour window and the name is held forever
after. Once the process is boring, change the workflow's trigger to `release: { types: [published] }`.

**Locally**, the same layout builds with:

```sh
node npm/build-packages.mjs --version 0.18.0 --bins <dir> --scope @petukhovart
```

where `<dir>` holds one subdirectory per rust target with the built binary. Output lands in `npm/dist/`
(gitignored). The script chmods the binaries — GitHub artifact downloads drop the executable bit — and
smoke-tests the launcher against this machine's own platform package before it finishes.

## After a release

Upgrading while a daemon is running is safe: sessions keep running on the old binary until someone runs
`pacer kill`. `pacer upgrade` says so, and `install.sh` refuses to clobber a local `cargo build` unless
passed `--force`.
