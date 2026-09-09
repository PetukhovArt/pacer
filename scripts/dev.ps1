# Isolated dev instance for Windows: its own runtime dir, database and
# daemon, so a half-finished build never drives your real sessions. This is
# the Windows counterpart of `make dev` (the Makefile is unix-only: shasum,
# /tmp, ps, kill, no .exe suffix).
#
#   powershell -ExecutionPolicy Bypass -File scripts\dev.ps1
#
# Run it from a terminal OUTSIDE pacer, in its own window. Quit the TUI with
# `q` and run it again to pick up a rebuild.
#
#   -Agent cmd.exe   stub agents out, so nothing spawns a real claude
#   -Reset           wipe this checkout's dev database first

param([string]$Agent, [switch]$Reset)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# Key the instance to the checkout, so a worktree gets its own daemon, port
# and database instead of silently driving this one's.
$sha = [System.Security.Cryptography.SHA1]::Create()
$slot = (($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($PWD.Path)) |
    ForEach-Object { $_.ToString('x2') }) -join '').Substring(0, 8)

$env:PACER_RUNTIME_DIR = Join-Path $env:TEMP "pacer-dev-$slot"
$env:PACER_DATA_DIR = Join-Path $HOME ".pacer-dev\$(Split-Path -Leaf $PWD)-$slot"
if ($Agent) { $env:PACER_AGENT_CMD = $Agent }

$bin = 'target\debug\pacer.exe'
cargo build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Load-bearing: the daemon detaches, so a dev daemon from the previous run
# outlived its TUI and is still executing the OLD code. Connecting to it is
# precisely how "I rebuilt and my change isn't there" happens.
& $bin kill | Out-Null
if ($Reset) { Remove-Item -Recurse -Force $env:PACER_DATA_DIR -ErrorAction SilentlyContinue }

Write-Host "dev instance [$(Split-Path -Leaf $PWD)] -> runtime $env:PACER_RUNTIME_DIR, data $env:PACER_DATA_DIR"
& $bin
& $bin kill | Out-Null
