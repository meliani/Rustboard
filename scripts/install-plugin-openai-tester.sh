#!/usr/bin/env bash
# install-plugin-openai-tester.sh
# Builds the plugin and installs it into the plugins/bin/ directory.
#
# Usage:
#   ./scripts/install-plugin-openai-tester.sh
#   ./scripts/install-plugin-openai-tester.sh --release   # optimised build

set -euo pipefail

RELEASE=0
for arg in "$@"; do
  [[ "$arg" == "--release" ]] && RELEASE=1
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ $RELEASE -eq 1 ]]; then
  echo "Building plugin-openai-tester (release)..."
  cargo build --release -p plugin-openai-tester
  BIN="$ROOT/target/release/plugin-openai-tester"
else
  echo "Building plugin-openai-tester (debug)..."
  cargo build -p plugin-openai-tester
  BIN="$ROOT/target/debug/plugin-openai-tester"
fi

if [[ ! -f "$BIN" ]]; then
  echo "Error: binary not found at $BIN" >&2
  exit 1
fi

DEST="$ROOT/plugins/bin"
mkdir -p "$DEST"

TARGET="$DEST/plugin-openai-tester"
cp -f "$BIN" "$TARGET"
chmod +x "$TARGET"

echo "Installed: $TARGET"
echo ""
echo "Usage via dashboard API:"
echo '  POST /plugins/exec'
echo '  { "name": "plugin-openai-tester",'
echo '    "input": { "api_key": "sk-...", "base_url": "https://api.openai.com/v1" } }'
echo ""
echo "Test locally:"
echo "  echo '{\"api_key\":\"sk-...\"}' | $TARGET"
