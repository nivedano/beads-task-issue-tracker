<#
.SYNOPSIS
Builds a production release of Beads Task-Issue Tracker.

.DESCRIPTION
Validates project versions and required tools, installs locked frontend
dependencies, and builds the optimized Tauri release executable. Checks and
installer bundles are opt-in so the default build does not populate Cargo's
debug target directory.

.PARAMETER SkipInstall
Skips the locked frontend dependency installation.

.PARAMETER SkipChecks
Skips the Vitest, Vue TypeScript, and Rust test gates. Enabled by default; pass
-SkipChecks:$false to run the checks.

.PARAMETER ExecutableOnly
Builds only the release executable instead of platform installer bundles.
Enabled by default; pass -ExecutableOnly:$false to build installer bundles.

.EXAMPLE
./tools/release.ps1

.EXAMPLE
./tools/release.ps1 -SkipChecks:$false -ExecutableOnly:$false
#>

[CmdletBinding()]
param(
    [switch]$SkipInstall,
    [switch]$SkipChecks = $true,
    [switch]$ExecutableOnly = $true
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-BunExecutable {
    $command = Get-Command bun -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $userProfile = [Environment]::GetFolderPath('UserProfile')
    $localAppData = [Environment]::GetFolderPath('LocalApplicationData')
    $candidates = @(
        (Join-Path $userProfile '.bun\bin\bun.exe'),
        (Join-Path $localAppData 'Microsoft\WinGet\Links\bun.exe')
    )

    $wingetPackages = Join-Path $localAppData 'Microsoft\WinGet\Packages'
    if (Test-Path -LiteralPath $wingetPackages) {
        $candidates += Get-ChildItem -Path (Join-Path $wingetPackages 'Oven-sh.Bun_*') -Directory -ErrorAction SilentlyContinue |
            ForEach-Object { Join-Path $_.FullName 'bun-windows-x64\bun.exe' }
    }

    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return $candidate
        }
    }

    throw 'Bun was not found. Install Bun and ensure bun is available on PATH.'
}

function Assert-Command {
    param([Parameter(Mandatory)][string]$Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found on PATH."
    }
}

function Invoke-NativeCommand {
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [Parameter(Mandatory)][string[]]$ArgumentList,
        [Parameter(Mandatory)][string]$Step
    )

    Write-Host ''
    Write-Host "==> $Step" -ForegroundColor Cyan
    & $FilePath @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "$Step failed with exit code $LASTEXITCODE."
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$bunWasOnPath = [bool](Get-Command bun -ErrorAction SilentlyContinue)
$bun = Resolve-BunExecutable
$temporaryBunDirectory = $null

# WinGet package directories can be executable by absolute path while still
# being unavailable to child-process command lookup. Give Tauri's nested
# beforeBuildCommand a short, ordinary PATH entry in that case.
if (-not $bunWasOnPath) {
    $temporaryBunDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ('beads-release-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $temporaryBunDirectory | Out-Null
    $temporaryBun = Join-Path $temporaryBunDirectory 'bun.exe'
    Copy-Item -LiteralPath $bun -Destination $temporaryBun
    $bun = $temporaryBun
}

$bunDirectory = Split-Path -Parent $bun
$pathEntries = $env:PATH -split [System.IO.Path]::PathSeparator
if ($bunDirectory -notin $pathEntries) {
    $env:PATH = $bunDirectory + [System.IO.Path]::PathSeparator + $env:PATH
}

Push-Location $repoRoot
try {
    Assert-Command node
    Assert-Command cargo

    $releaseExecutable = Join-Path $repoRoot 'src-tauri\target\release\beads-issue-tracker.exe'
    $runningReleaseProcesses = @(
        Get-Process -Name 'beads-issue-tracker' -ErrorAction SilentlyContinue |
            Where-Object {
                try { $_.Path -eq $releaseExecutable }
                catch { $false }
            }
    )
    if ($runningReleaseProcesses.Count -gt 0) {
        $processIds = ($runningReleaseProcesses.Id | Sort-Object) -join ', '
        throw "Close the running release app before building. Process ID(s): $processIds."
    }

    $packageVersion = (Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json).version
    $tauriVersion = (Get-Content -LiteralPath 'src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json).version
    $cargoManifest = Get-Content -LiteralPath 'src-tauri\Cargo.toml' -Raw
    $cargoVersionMatch = [regex]::Match($cargoManifest, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $cargoVersionMatch.Success) {
        throw 'Could not read the package version from src-tauri/Cargo.toml.'
    }
    $cargoVersion = $cargoVersionMatch.Groups[1].Value

    if (($packageVersion -ne $tauriVersion) -or ($packageVersion -ne $cargoVersion)) {
        throw "Version mismatch: package.json=$packageVersion, tauri.conf.json=$tauriVersion, Cargo.toml=$cargoVersion."
    }

    Write-Host "Building Beads Task-Issue Tracker v$packageVersion" -ForegroundColor Green
    Write-Host "Bun: $bun"

    if (-not $SkipInstall) {
        Invoke-NativeCommand -FilePath $bun -ArgumentList @('install', '--frozen-lockfile') -Step 'Install locked frontend dependencies'
    }

    if (-not $SkipChecks) {
        Invoke-NativeCommand -FilePath $bun -ArgumentList @('run', 'test') -Step 'Run frontend tests'
        Invoke-NativeCommand -FilePath $bun -ArgumentList @('x', 'vue-tsc', '--noEmit') -Step 'Run Vue TypeScript checks'
        Invoke-NativeCommand -FilePath 'cargo' -ArgumentList @('test', '--manifest-path', 'src-tauri/Cargo.toml') -Step 'Run Rust tests'
    }

    $buildScript = if ($ExecutableOnly) { 'tauri:build:exe' } else { 'tauri:build' }
    $buildDescription = if ($ExecutableOnly) { 'Build release executable' } else { 'Build release bundles' }
    Invoke-NativeCommand -FilePath $bun -ArgumentList @('run', $buildScript) -Step $buildDescription

    $targetDirectory = Join-Path $repoRoot 'src-tauri\target\release'
    $artifactPatterns = if ($ExecutableOnly) {
        @('beads-issue-tracker', 'beads-issue-tracker.exe')
    }
    else {
        @('*.msi', '*-setup.exe', '*.dmg', '*.deb', '*.AppImage')
    }

    $artifacts = foreach ($pattern in $artifactPatterns) {
        Get-ChildItem -Path $targetDirectory -Filter $pattern -File -Recurse -ErrorAction SilentlyContinue
    }
    $artifacts = @($artifacts | Sort-Object FullName -Unique)

    if ($artifacts.Count -eq 0) {
        throw "The build succeeded but no release artifacts were found under $targetDirectory."
    }

    Write-Host ''
    Write-Host "Release v$packageVersion built successfully:" -ForegroundColor Green
    foreach ($artifact in $artifacts) {
        $relativePath = Resolve-Path -LiteralPath $artifact.FullName -Relative
        $sizeMiB = [math]::Round($artifact.Length / 1MB, 2)
        Write-Host "  $relativePath ($sizeMiB MiB)"
    }
}
finally {
    Pop-Location
    if ($temporaryBunDirectory) {
        $temporaryBun = Join-Path $temporaryBunDirectory 'bun.exe'
        if (Test-Path -LiteralPath $temporaryBun) {
            Remove-Item -LiteralPath $temporaryBun -Force
        }
        if (Test-Path -LiteralPath $temporaryBunDirectory) {
            Remove-Item -LiteralPath $temporaryBunDirectory -Force
        }
    }
}
