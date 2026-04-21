# install-plugin-openai-tester.ps1
# Builds the plugin and installs it into the plugins/ directory so the
# dashboard can discover and invoke it.
#
# Usage:
#   .\scripts\install-plugin-openai-tester.ps1
#   .\scripts\install-plugin-openai-tester.ps1 -Release   # optimised build

param(
    [switch]$Release
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent

Push-Location $root
try {
    if ($Release) {
        Write-Host "Building plugin-openai-tester (release)..." -ForegroundColor Cyan
        cargo build --release -p plugin-openai-tester
        $bin = Join-Path $root "target\release\plugin-openai-tester.exe"
    } else {
        Write-Host "Building plugin-openai-tester (debug)..." -ForegroundColor Cyan
        cargo build -p plugin-openai-tester
        $bin = Join-Path $root "target\debug\plugin-openai-tester.exe"
    }

    if (-not (Test-Path $bin)) {
        Write-Error "Build succeeded but binary not found at: $bin"
        exit 1
    }

    $dest = Join-Path $root "plugins\bin"
    if (-not (Test-Path $dest)) {
        New-Item -ItemType Directory -Path $dest | Out-Null
    }

    $target = Join-Path $dest "plugin-openai-tester.exe"
    Copy-Item -Force $bin $target
    Write-Host "Installed: $target" -ForegroundColor Green
    Write-Host ""
    Write-Host "Usage via dashboard API:" -ForegroundColor Yellow
    Write-Host '  POST /plugins/exec'
    Write-Host '  { "name": "plugin-openai-tester.exe",'
    Write-Host '    "input": { "api_key": "sk-...", "base_url": "https://api.openai.com/v1" } }'
    Write-Host ""
    Write-Host "Test locally:"
    Write-Host "  echo '{\"api_key\":\"sk-...\"}' | $target"
} finally {
    Pop-Location
}
