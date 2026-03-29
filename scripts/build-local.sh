#!/bin/bash
set -e

echo "Building project in release mode..."
cargo build --release

echo "Copying binaries to root directory..."
if [ -f "target/release/core" ]; then
    cp target/release/core rush-core
    echo "Created rush-core"
fi

if [ -f "target/release/cli" ]; then
    cp target/release/cli rush-cli
    echo "Created rush-cli"
fi

echo "Build complete! You can now run the app using: ./rush-core config/services.example.yaml"
echo "Or use the CLI: ./rush-cli"
