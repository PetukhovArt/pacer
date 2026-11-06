# Isolated dev instance for Windows: its own runtime dir, database and
# daemon, so a half-finished build never drives your real sessions. This is
# the Windows counterpart of `make dev` (the Makefile is unix-only: shasum,
# /tmp, ps, kill, no .exe suffix).
#
#   powershell -ExecutionPolicy Bypass -File scripts\dev.ps1
#
# Run it from a terminal OUTSIDE berth, in its own window. Quit the TUI with
# `q` and run it again to pick up a rebuild.
#
#   -Agent cmd.exe   stub agents out, so nothing spawns a real claude
#   -Reset           wipe this checkout's dev database first
#   -Desktop         run the desktop app (`pnpm tauri dev`) instead of the TUI,
#                    against a dev daemon this script starts and stops; the
#                    app's own dir (BERTH_HOME) lives in the dev data dir too
#   -CdpPort 9334    WebView2 remote-debugging port for the desktop app

param([string]$Agent, [switch]$Reset, [switch]$Desktop, [int]$CdpPort = 9334)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# Key the instance to the checkout, so a worktree gets its own daemon, port
# and database instead of silently driving this one's.
$sha = [System.Security.Cryptography.SHA1]::Create()
$slot = (($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($PWD.Path)) |
    ForEach-Object { $_.ToString('x2') }) -join '').Substring(0, 8)

$env:BERTH_RUNTIME_DIR = Join-Path $env:TEMP "berth-dev-$slot"
$env:BERTH_DATA_DIR = Join-Path $HOME ".berth-dev\$(Split-Path -Leaf $PWD)-$slot"
if ($Agent) { $env:BERTH_AGENT_CMD = $Agent }

$bin = 'target\debug\berth.exe'
cargo build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Load-bearing: the daemon detaches, so a dev daemon from the previous run
# outlived its TUI and is still executing the OLD code. Connecting to it is
# precisely how "I rebuilt and my change isn't there" happens.
& $bin kill | Out-Null
if ($Reset) { Remove-Item -Recurse -Force $env:BERTH_DATA_DIR -ErrorAction SilentlyContinue }

Write-Host "dev instance [$(Split-Path -Leaf $PWD)] -> runtime $env:BERTH_RUNTIME_DIR, data $env:BERTH_DATA_DIR"
if ($Desktop) {
    # The desktop app connects to a daemon but never starts one, and its own
    # settings, connectors and secrets (a file, not the OS keychain) go to
    # BERTH_HOME instead of your real ~/.berth.
    $env:BERTH_HOME = Join-Path $env:BERTH_DATA_DIR 'home'
    $env:BERTH_SECRETS = 'file'
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$CdpPort"
    Start-Process -FilePath $bin -ArgumentList 'daemon' -WindowStyle Hidden
    pnpm --filter @berth/desktop tauri dev
} else {
    & $bin
}
& $bin kill | Out-Null
