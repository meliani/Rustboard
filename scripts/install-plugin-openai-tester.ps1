# install-plugin-openai-tester.ps1
# Builds the Extism WASM plugin and installs it into plugins/bin/.
#
# Usage:
#   .\scripts\install-plugin-openai-tester.ps1           # release build (default)
#   .\scripts\install-plugin-openai-tester.ps1 -Debug    # debug build

param(
    [switch]$Debug
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent

Push-Location $root
try {
    # Ensure the wasm32-wasip1 target is available
    rustup target add wasm32-wasip1

    if ($Debug) {
        Write-Host "Building plugin-openai-tester (debug, wasm32-wasip1)..." -ForegroundColor Cyan
        cargo build -p plugin-openai-tester --target wasm32-wasip1
        $wasm = Join-Path $root "target\wasm32-wasip1\debug\plugin_openai_tester.wasm"
    } else {
        Write-Host "Building plugin-openai-tester (release, wasm32-wasip1)..." -ForegroundColor Cyan
        cargo build --release -p plugin-openai-tester --target wasm32-wasip1
        $wasm = Join-Path $root "target\wasm32-wasip1\release\plugin_openai_tester.wasm"
    }

    if (-not (Test-Path $wasm)) {
        Write-Error "Build succeeded but WASM module not found at: $wasm"
        exit 1
    }

    $dest = Join-Path $root "plugins\bin"
    New-Item -ItemType Directory -Force -Path $dest | Out-Null

    $target = Join-Path $dest "plugin-openai-tester.wasm"
    Copy-Item -Force $wasm $target
    Write-Host "Installed: $target" -ForegroundColor Green
    Write-Host ""
    Write-Host "Usage via dashboard API:" -ForegroundColor Yellow
    Write-Host '  POST /plugins/exec'
    Write-Host '  { "name": "plugin-openai-tester",'
    Write-Host '    "input": { "api_key": "sk-...", "base_url": "https://api.openai.com/v1" } }'
} finally {
    Pop-Location
}
