#!/usr/bin/env bash
# install-plugin-openai-tester.sh
# Builds the Extism WASM plugin and installs it into plugins/bin/.
#
# Usage:
#   ./scripts/install-plugin-openai-tester.sh           # release build (default)
#   ./scripts/install-plugin-openai-tester.sh --debug   # debug build

set -euo pipefail

PROFILE=release
for arg in "$@"; do
  [[ "$arg" == "--debug" ]] && PROFILE=debug
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Ensure the wasm32-wasip1 target is installed
rustup target add wasm32-wasip1

if [[ "$PROFILE" == "release" ]]; then
  echo "Building plugin-openai-tester (release, wasm32-wasip1)..."
  cargo build --release -p plugin-openai-tester --target wasm32-wasip1
  WASM="$ROOT/target/wasm32-wasip1/release/plugin_openai_tester.wasm"
else
  echo "Building plugin-openai-tester (debug, wasm32-wasip1)..."
  cargo build -p plugin-openai-tester --target wasm32-wasip1
  WASM="$ROOT/target/wasm32-wasip1/debug/plugin_openai_tester.wasm"
fi

if [[ ! -f "$WASM" ]]; then
  echo "Error: WASM module not found at $WASM" >&2
  exit 1
fi

DEST="$ROOT/plugins/bin"
mkdir -p "$DEST"

TARGET="$DEST/plugin-openai-tester.wasm"
cp -f "$WASM" "$TARGET"

echo "Installed: $TARGET"
echo ""
echo "Usage via dashboard API:"
echo '  POST /plugins/exec'
echo '  { "name": "plugin-openai-tester",'
echo '    "input": { "api_key": "sk-...", "base_url": "https://api.openai.com/v1" } }'
