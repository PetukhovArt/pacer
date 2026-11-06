param(
    [string]$Repo,
    [string]$Root,
    [string]$Ref = 'main',
    [string]$ShortcutDir,
    [switch]$Force,
    [switch]$Launch,
    [switch]$Rollback,
    [switch]$UpdateDaemon
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$utf8 = [Text.UTF8Encoding]::new($false)

if (-not $Root) { $Root = if ($env:BERTH_APP_ROOT) { $env:BERTH_APP_ROOT } else { Join-Path $env:LOCALAPPDATA 'Berth' } }
if (-not $ShortcutDir) { $ShortcutDir = if ($env:BERTH_START_MENU) { $env:BERTH_START_MENU } else { Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs' } }

$appDir = Join-Path $Root 'app'
$buildDir = Join-Path $Root 'build'
$current = Join-Path $appDir 'current'
$previous = Join-Path $appDir 'previous'
$statusPath = Join-Path $buildDir 'status.json'
$exeName = 'berth-desktop.exe'

function Read-Build([string]$dir) {
    $file = Join-Path $dir 'build.json'
    if (Test-Path -LiteralPath $file) { Get-Content -Raw -LiteralPath $file | ConvertFrom-Json }
}

function Get-LinkTarget([string]$link) {
    if (-not (Test-Path -LiteralPath $link)) { return $null }
    $target = [string](Get-Item -LiteralPath $link).Target
    if ($target) { $target } else { $null }
}

function Set-Link([string]$link, [string]$target) {
    if (Test-Path -LiteralPath $link) { [IO.Directory]::Delete($link) }
    New-Item -ItemType Junction -Path $link -Target $target | Out-Null
}

function Set-Shortcut {
    New-Item -ItemType Directory -Force -Path $ShortcutDir | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut((Join-Path $ShortcutDir 'Berth.lnk'))
    $link.TargetPath = Join-Path $current $exeName
    $link.WorkingDirectory = $current
    $link.IconLocation = "$(Join-Path $current $exeName),0"
    $link.Description = 'Berth'
    $link.Save()
}

function Start-Berth {
    Start-Process -FilePath (Join-Path $current $exeName) -WorkingDirectory $current
}

if ($Rollback) {
    $now = Get-LinkTarget $current
    $then = Get-LinkTarget $previous
    if (-not $then -or -not (Test-Path -LiteralPath (Join-Path $then $exeName))) { throw "no previous build under $appDir" }
    Set-Link $current $then
    if ($now) { Set-Link $previous $now }
    $build = Read-Build $current
    Write-Host "current build: $($build.buildId) $($build.subject)"
    if ($Launch) { Start-Berth }
    exit 0
}

if ($UpdateDaemon) {
    $built = Join-Path $current 'berth.exe'
    if (-not (Test-Path -LiteralPath $built)) { throw "no berth.exe in $current" }
    $prefix = if ($env:BERTH_BIN_DIR) { $env:BERTH_BIN_DIR } else { Join-Path $HOME '.cargo\bin' }
    New-Item -ItemType Directory -Force -Path $prefix | Out-Null
    $installed = Join-Path $prefix 'berth.exe'
    $old = Join-Path $prefix 'berth.old.exe'
    if (Test-Path -LiteralPath $old) { Remove-Item -LiteralPath $old -Force -ErrorAction SilentlyContinue }
    if (Test-Path -LiteralPath $installed) { Move-Item -LiteralPath $installed -Destination $old -Force }
    Copy-Item -LiteralPath $built -Destination $installed
    & $installed --version
    & $installed kill
    Start-Process -FilePath $installed -ArgumentList 'daemon' -WindowStyle Hidden
    Write-Host "daemon restarted from $installed"
    exit 0
}

if (-not $Repo) {
    $checkout = Join-Path $PSScriptRoot '..'
    $own = Read-Build $PSScriptRoot
    if (Test-Path -LiteralPath (Join-Path $checkout '.git')) { $Repo = (Resolve-Path $checkout).Path }
    elseif ($own) { $Repo = $own.sourceRepo }
    else { throw 'pass -Repo <path to the berth checkout>' }
}
$Repo = (Resolve-Path -LiteralPath $Repo).Path

New-Item -ItemType Directory -Force -Path $buildDir, $appDir, (Join-Path $buildDir 'logs') | Out-Null
$status = [ordered]@{ state = 'running'; step = 'start'; pid = $PID; repo = $Repo; startedAt = (Get-Date).ToString('o') }

function Save-Status {
    $temp = "$statusPath.tmp"
    [IO.File]::WriteAllText($temp, ($status | ConvertTo-Json -Depth 4), $utf8)
    Move-Item -LiteralPath $temp -Destination $statusPath -Force
}

function Set-Step([string]$step) {
    $status.step = $step
    Save-Status
    Add-Log "== $step"
}

function Add-Log([string]$line) {
    [IO.File]::AppendAllText($log, "$line`r`n", $utf8)
}

function Invoke-Logged([string]$exe, [string[]]$arguments) {
    Add-Log "> $exe $($arguments -join ' ')"
    $saved = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $exe @arguments 2>&1 | ForEach-Object { Add-Log "$_" }
    $code = $LASTEXITCODE
    $ErrorActionPreference = $saved
    if ($code -ne 0) { throw "$exe $($arguments -join ' ') exited with code $code" }
}

$log = Join-Path $buildDir "logs\$(Get-Date -Format 'yyyyMMdd-HHmmss').log"
$status.log = $log
Save-Status

try {
    $commit = (git -C $Repo rev-parse --verify "refs/heads/$Ref^{commit}").Trim()
    if ($LASTEXITCODE -ne 0 -or -not $commit) { throw "no branch $Ref in $Repo" }
    $subject = (git -C $Repo log -1 --format=%s $commit) -join ' '
    $status.commit = $commit
    $status.subject = $subject
    Save-Status

    $installedBuild = Read-Build $current
    if (-not $Force -and $installedBuild -and $installedBuild.commit -eq $commit) {
        $status.state = 'done'
        $status.upToDate = $true
        $status.buildId = $installedBuild.buildId
        $status.finishedAt = (Get-Date).ToString('o')
        Save-Status
        Write-Host "already up to date: $($installedBuild.buildId)"
        if ($Launch) { Start-Berth }
        exit 0
    }

    Set-Step 'export'
    $src = Join-Path $buildDir 'src'
    New-Item -ItemType Directory -Force -Path $src | Out-Null
    $gitDir = (git -C $Repo rev-parse --absolute-git-dir).Trim()
    $env:GIT_INDEX_FILE = Join-Path $buildDir 'src.index'
    try {
        Invoke-Logged 'git' @("--git-dir=$gitDir", "--work-tree=$src", 'read-tree', '--reset', '-u', $commit)
    } finally {
        Remove-Item Env:\GIT_INDEX_FILE
    }

    $env:CARGO_TARGET_DIR = Join-Path $buildDir 'target'
    Push-Location $src
    try {
        Set-Step 'deps'
        Invoke-Logged 'pnpm' @('install', '--frozen-lockfile', '--config.confirmModulesPurge=false')
        Set-Step 'tokens'
        Invoke-Logged 'pnpm' @('build:tokens')
        Set-Step 'app'
        Invoke-Logged 'pnpm' @('--filter', '@berth/desktop', 'exec', 'tauri', 'build', '--no-bundle')
        Set-Step 'daemon'
        Invoke-Logged 'cargo' @('build', '--release', '--locked', '-p', 'berth')
    } finally {
        Pop-Location
    }

    Set-Step 'install'
    $release = Join-Path $env:CARGO_TARGET_DIR 'release'
    $protocolLine = Select-String -LiteralPath (Join-Path $src 'crates\berth-core\src\protocol.rs') -Pattern 'pub const PROTOCOL_VERSION: u32 = (\d+);' | Select-Object -First 1
    $protocol = [int]$protocolLine.Matches[0].Groups[1].Value
    $buildId = "$(Get-Date -Format 'yyyyMMdd-HHmmss')-$($commit.Substring(0, 8))"
    $target = Join-Path $appDir $buildId
    $staging = "$target.partial"
    if (Test-Path -LiteralPath $staging) { Remove-Item -Recurse -Force -LiteralPath $staging }
    New-Item -ItemType Directory -Force -Path $staging | Out-Null
    Copy-Item -LiteralPath (Join-Path $release $exeName) -Destination $staging
    Copy-Item -LiteralPath (Join-Path $release 'berth.exe') -Destination $staging
    Copy-Item -LiteralPath (Join-Path $src 'scripts\install-app.ps1') -Destination $staging
    $build = [ordered]@{ buildId = $buildId; commit = $commit; subject = $subject; sourceRepo = $Repo; builtAt = (Get-Date).ToString('o'); protocol = $protocol }
    [IO.File]::WriteAllText((Join-Path $staging 'build.json'), ($build | ConvertTo-Json), $utf8)
    Rename-Item -LiteralPath $staging -NewName $buildId

    $was = Get-LinkTarget $current
    Set-Link $current $target
    if ($was) { Set-Link $previous $was }
    Set-Shortcut

    $keep = @($target, $was) | Where-Object { $_ } | ForEach-Object { [IO.Path]::GetFullPath($_).TrimEnd('\') }
    $running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path } | ForEach-Object { [IO.Path]::GetDirectoryName($_.Path) })
    Get-ChildItem -LiteralPath $appDir -Directory | Where-Object {
        $_.Name -notin @('current', 'previous') -and -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint)
    } | ForEach-Object {
        $full = $_.FullName.TrimEnd('\')
        if ($keep -contains $full -or $running -contains $full) { return }
        Remove-Item -Recurse -Force -LiteralPath $full -ErrorAction SilentlyContinue
    }

    $status.state = 'done'
    $status.step = 'install'
    $status.buildId = $buildId
    $status.protocol = $protocol
    $status.finishedAt = (Get-Date).ToString('o')
    Save-Status
    Add-Log "installed $buildId into $target"
    Write-Host "installed $buildId ($subject) into $target"
    if ($Launch) { Start-Berth }
} catch {
    $status.state = 'failed'
    $status.error = "$_"
    $status.finishedAt = (Get-Date).ToString('o')
    Save-Status
    Add-Log "failed: $_"
    Write-Error "build failed: $_ (log: $log)" -ErrorAction Continue
    exit 1
}
