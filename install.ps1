# Rustboard installer — Windows PowerShell
# Usage (run in an elevated or user PowerShell):
#   irm https://raw.githubusercontent.com/meliani/Rustboard/main/install.ps1 | iex
#
# Or specify a custom install directory:
#   $env:RUSTBOARD_DIR = "$HOME\.rustboard\bin"; irm https://raw.githubusercontent.com/meliani/Rustboard/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo    = "meliani/Rustboard"
$InstDir = if ($env:RUSTBOARD_DIR) { $env:RUSTBOARD_DIR } else { "$env:LOCALAPPDATA\Rustboard\bin" }
$Bins    = @("rustboard-core", "rustboard-cli")
$Suffix  = "windows-x86_64"

function Write-Info  { param($m) Write-Host "[rustboard] $m" -ForegroundColor Cyan }
function Write-Ok    { param($m) Write-Host "[rustboard] $m" -ForegroundColor Green }
function Write-Fail  { param($m) Write-Host "[rustboard] $m" -ForegroundColor Red; exit 1 }

# ── resolve latest release ────────────────────────────────────────────────────
Write-Info "Fetching latest release from GitHub..."
try {
    $release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
} catch {
    Write-Fail "Could not fetch release info: $_"
}

$Tag = $release.tag_name
if (-not $Tag) { Write-Fail "Could not determine latest release tag." }
Write-Info "Latest release: $Tag"

# ── download & install ────────────────────────────────────────────────────────
if (-not (Test-Path $InstDir)) {
    New-Item -ItemType Directory -Path $InstDir -Force | Out-Null
}

foreach ($Bin in $Bins) {
    $Asset = "${Bin}-${Suffix}.exe"
    $Url   = "https://github.com/$Repo/releases/download/$Tag/$Asset"
    $Dest  = Join-Path $InstDir "${Bin}.exe"

    Write-Info "Downloading $Asset..."
    try {
        Invoke-WebRequest -Uri $Url -OutFile $Dest -UseBasicParsing
    } catch {
        Write-Fail "Failed to download $Asset from $Url`n$_"
    }
    Write-Ok "Installed: $Dest"
}

# ── add to user PATH if needed ────────────────────────────────────────────────
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$UserPath;$InstDir", "User")
    Write-Ok "Added $InstDir to your user PATH (restart terminal to apply)"
} else {
    Write-Info "$InstDir is already in PATH"
}

# ── summary ───────────────────────────────────────────────────────────────────
Write-Ok "Rustboard $Tag installed to $InstDir"
Write-Host ""
Write-Host "  Run the dashboard:"
Write-Host "    rustboard-core.exe config\services.yaml"
Write-Host ""
Write-Host "  Use the CLI:"
Write-Host "    rustboard-cli.exe list"
Write-Host ""
