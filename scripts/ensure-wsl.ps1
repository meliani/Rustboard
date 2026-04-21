<#
Ensure commands are executed inside WSL when running from Windows PowerShell.

Usage:
  .\scripts\ensure-wsl.ps1 cargo build --workspace
  .\scripts\ensure-wsl.ps1 cargo run -p core -- config/services.example.yaml
#>


function Write-Usage {
    Write-Host "Usage: .\scripts\ensure-wsl.ps1 <command...>"
    Write-Host "Example: .\scripts\ensure-wsl.ps1 cargo run -p core -- config/services.example.yaml"
}

# If not Windows (e.g., already in WSL), run locally
if ($env:OS -notlike "*Windows*") {
    if ($args.Count -eq 0) { Write-Usage; exit 0 }
    & $args
    exit $LASTEXITCODE
}

# On Windows: ensure WSL exists
$wsl = Get-Command wsl -ErrorAction SilentlyContinue
if (-not $wsl) {
    Write-Host "WSL not found. Install WSL: https://learn.microsoft.com/windows/wsl/install" -ForegroundColor Red
    exit 1
}

if ($args.Count -eq 0) { Write-Usage; exit 0 }

$command = $args -join ' '
Write-Host "Running in WSL: $command"
& wsl bash -lc $command
exit $LASTEXITCODE
