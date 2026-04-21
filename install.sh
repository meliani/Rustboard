#!/usr/bin/env bash
# Rustboard installer — Linux & macOS
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/meliani/Rustboard/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/meliani/Rustboard/main/install.sh | bash -s -- --dir ~/.local/bin

set -euo pipefail

REPO="meliani/Rustboard"
INSTALL_DIR="${RUSTBOARD_DIR:-/usr/local/bin}"
BINARIES=("rustboard-core" "rustboard-cli")

# ── helpers ──────────────────────────────────────────────────────────────────
info()  { printf "\033[1;34m[rustboard]\033[0m %s\n" "$*"; }
ok()    { printf "\033[1;32m[rustboard]\033[0m %s\n" "$*"; }
err()   { printf "\033[1;31m[rustboard]\033[0m %s\n" "$*" >&2; exit 1; }

# ── parse args ───────────────────────────────────────────────────────────────
for arg in "$@"; do
  case $arg in
    --dir=*) INSTALL_DIR="${arg#*=}" ;;
    --dir)   shift; INSTALL_DIR="$1" ;;
  esac
done

# ── detect platform ───────────────────────────────────────────────────────────
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  linux*)  PLATFORM="linux" ;;
  darwin*) PLATFORM="macos" ;;
  *)       err "Unsupported OS: $OS — please download manually from https://github.com/$REPO/releases" ;;
esac

case "$ARCH" in
  x86_64)          ARCH_SUFFIX="x86_64" ;;
  arm64|aarch64)   ARCH_SUFFIX="aarch64" ;;
  *)               err "Unsupported architecture: $ARCH" ;;
esac

SUFFIX="${PLATFORM}-${ARCH_SUFFIX}"
info "Detected platform: $SUFFIX"

# ── resolve latest release ───────────────────────────────────────────────────
info "Fetching latest release from GitHub..."
LATEST_URL="https://api.github.com/repos/${REPO}/releases/latest"

if command -v curl &>/dev/null; then
  RELEASE_JSON=$(curl -fsSL "$LATEST_URL")
elif command -v wget &>/dev/null; then
  RELEASE_JSON=$(wget -qO- "$LATEST_URL")
else
  err "Neither curl nor wget found. Install one and retry."
fi

TAG=$(printf '%s' "$RELEASE_JSON" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name":[[:space:]]*"\(.*\)".*/\1/')
[ -z "$TAG" ] && err "Could not determine latest release tag. Check https://github.com/$REPO/releases"
info "Latest release: $TAG"

# ── download & install ───────────────────────────────────────────────────────
mkdir -p "$INSTALL_DIR"

for BIN in "${BINARIES[@]}"; do
  ASSET="${BIN}-${SUFFIX}"
  URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
  DEST="${INSTALL_DIR}/${BIN}"

  info "Downloading $ASSET..."
  if command -v curl &>/dev/null; then
    curl -fsSL "$URL" -o "$DEST"
  else
    wget -qO "$DEST" "$URL"
  fi
  chmod +x "$DEST"
  ok "Installed: $DEST"
done

# ── summary ──────────────────────────────────────────────────────────────────
ok "Rustboard $TAG installed to $INSTALL_DIR"
echo ""
echo "  Run the dashboard:"
echo "    rustboard-core config/services.yaml"
echo ""
echo "  Use the CLI:"
echo "    rustboard-cli list"
echo ""

if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  printf "\033[1;33m[rustboard]\033[0m %s is not in your PATH.\n" "$INSTALL_DIR"
  echo "  Add this to your shell profile:"
  echo "    export PATH=\"$INSTALL_DIR:\$PATH\""
fi
