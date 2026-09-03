# Releasing

Three channels, one set of binaries:

| Channel | Who it serves | Built by |
|---|---|---|
| GitHub release | `install.sh`, `pacer upgrade`, manual downloads | `.github/workflows/release.yml`, on a version bump |
| npm | anyone with Node — and the only frictionless path on Windows | `.github/workflows/npm.yml`, called by the same run |
| `cargo install --git` | source builds, fallback when no asset matches | nothing to do |

crates.io is not a channel: the workspace pulls a patched `vt100` through `[patch.crates-io]`, and
`cargo publish` refuses a patched dependency. `cargo install --git` is unaffected — it builds the
workspace as it stands, vendored fork and all.

## Cutting a release

Bump `version` in the root `Cargo.toml` (`[workspace.package]`), run `cargo check` so `Cargo.lock`
follows, and land it on `main`. That is the whole procedure — **the version bump is the release**, so
do not bump it as part of unrelated work.

The `Release` workflow does the rest in one run:

1. `gate` reads the version and asks whether `v<version>` is already tagged. On every other push to
   `main` it is, and the run stops there — a push that changes no version costs one 30-second job.
2. `build` compiles five targets — macOS arm64/x64, Linux x64/arm64 (static musl), Windows x64 — and
   uploads two artifacts each: the archive `install.sh` downloads, and the bare binary npm wraps.
3. `release` tags the commit and attaches `pacer-<target>.tar.gz` (and `.zip` for Windows) to a GitHub
   release with generated notes.
4. `npm` calls `.github/workflows/npm.yml` with the new tag and the dry run off.

The tag is the record of what shipped, not the trigger. It has to be, because a tag pushed with
`GITHUB_TOKEN` starts no workflow — GitHub's guard against a workflow triggering itself. Tagging by
hand still works and re-releases that exact commit; a tag whose version disagrees with `Cargo.toml`
fails the gate before anything is built.

To check a cross-compile without releasing, run the workflow from the Actions tab
(`workflow_dispatch`) — it builds the matrix and skips the release and npm jobs.

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

Nothing to do — `Release` calls this workflow as its last job, with the new tag and the dry run off.
It downloads that release's assets — the same binaries, never a rebuild — lays out the packages, and
publishes the platform packages first and the launcher last, so the registry never holds a launcher
whose dependencies do not resolve yet.

Running it by hand from the Actions tab is for the two cases the automatic path does not cover:
re-publishing a release, and reading the file lists first. With **dry run** left on (the default) it
runs `npm pack --dry-run` over each package and publishes nothing.

Worth knowing before you run it with dry run off: `npm unpublish` is a 72-hour window, and the name is
held forever after.

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
