param ()

Write-Host "Building project in release mode..."
cargo build --release

Write-Host "Copying binaries to root directory..."
if (Test-Path "target\release\core.exe") {
    Copy-Item "target\release\core.exe" "rustboard-core.exe" -Force
    Write-Host "Created rustboard-core.exe"
}

if (Test-Path "target\release\cli.exe") {
    Copy-Item "target\release\cli.exe" "rustboard-cli.exe" -Force
    Write-Host "Created rustboard-cli.exe"
}

Write-Host "Build complete! You can now run the app directly using: .\rustboard-core.exe config\services.example.yaml"
Write-Host "Or use the CLI: .\rustboard-cli.exe"
