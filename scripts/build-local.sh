#!/bin/bash
set -e

echo "Building project in release mode..."
cargo build --release

echo "Copying binaries to root directory..."
if [ -f "target/release/core" ]; then
    cp target/release/core rustboard-core
    echo "Created rustboard-core"
fi

if [ -f "target/release/cli" ]; then
    cp target/release/cli rustboard-cli
    echo "Created rustboard-cli"
fi

echo "Build complete! You can now run the app using: ./rustboard-core config/services.example.yaml"
echo "Or use the CLI: ./rustboard-cli"
