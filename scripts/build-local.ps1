param ()

Write-Host "Building project in release mode..."
cargo build --release

Write-Host "Copying binaries to root directory..."
if (Test-Path "target\release\core.exe") {
    Copy-Item "target\release\core.exe" "rush-core.exe" -Force
    Write-Host "Created rush-core.exe"
}

if (Test-Path "target\release\cli.exe") {
    Copy-Item "target\release\cli.exe" "rush-cli.exe" -Force
    Write-Host "Created rush-cli.exe"
}

Write-Host "Build complete! You can now run the app directly using: .\rush-core.exe config\services.example.yaml"
Write-Host "Or use the CLI: .\rush-cli.exe"
