# Local cutover for Windows: build the release binary, install it to
# ~\.cargo\bin, and restart the daemon so the new build is what runs.
# The Makefile's `install`/`cycle` targets are unix-only (shasum, /tmp,
# no .exe suffix) — this script is their Windows counterpart.
#
# Run from a terminal OUTSIDE pacer: the cutover kills every session.
#
#   powershell -ExecutionPolicy Bypass -File scripts\update.ps1
#
# Windows locks a running executable, so the installed pacer.exe can't be
# overwritten while the daemon is up. Renaming it IS allowed — so we move
# the live binary aside to pacer.old.exe, copy the fresh build in, then
# kill the (still old-binary) daemon; the next `pacer` launch runs the
# new build. Build failures stop before anything is touched.

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$prefix = Join-Path $HOME '.cargo\bin'
$installed = Join-Path $prefix 'pacer.exe'
$old = Join-Path $prefix 'pacer.old.exe'
$built = 'target\release\pacer.exe'

cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# A leftover .old from the previous cutover is no longer running — drop it.
if (Test-Path $old) { Remove-Item $old }
if (Test-Path $installed) { Move-Item $installed $old }
Copy-Item $built $installed

& $installed --version

# Stop every session and the daemon (it's still the old binary). Exits 0
# with "no pacer daemon running" on a cold machine, so this always passes.
& $installed kill

Write-Host ''
Write-Host 'Updated. Start `pacer` to run the new build.'
