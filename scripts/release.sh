#!/usr/bin/env bash
# release.sh — bump version, tag, and push to trigger the GitHub release workflow
#
# Usage:
#   ./scripts/release.sh patch      # 0.1.0 → 0.1.1
#   ./scripts/release.sh minor      # 0.1.0 → 0.2.0
#   ./scripts/release.sh major      # 0.1.0 → 1.0.0
#   ./scripts/release.sh 1.2.3      # explicit version

set -euo pipefail

BUMP="${1:-patch}"

# ── get current version from git tags ────────────────────────────────────────
CURRENT=$(git tag --list "v*" --sort=-version:refname | head -1)
CURRENT="${CURRENT:-v0.0.0}"
CURRENT_VER="${CURRENT#v}"

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VER"

# ── calculate next version ────────────────────────────────────────────────────
case "$BUMP" in
  major) MAJOR=$((MAJOR+1)); MINOR=0; PATCH=0 ;;
  minor) MINOR=$((MINOR+1)); PATCH=0 ;;
  patch) PATCH=$((PATCH+1)) ;;
  v*.*.*) NEW_VER="${BUMP#v}" ;;
  *.*.*)  NEW_VER="$BUMP" ;;
  *) echo "Usage: $0 [major|minor|patch|x.y.z]"; exit 1 ;;
esac

NEW_VER="${NEW_VER:-${MAJOR}.${MINOR}.${PATCH}}"
TAG="v${NEW_VER}"

# ── guard: must be on main with a clean tree ──────────────────────────────────
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "main" ]]; then
  echo "ERROR: you must be on 'main' to release (current: $BRANCH)"
  exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "ERROR: working tree is dirty. Commit or stash changes first."
  exit 1
fi

git fetch origin --tags --quiet

if git rev-parse "$TAG" &>/dev/null; then
  echo "ERROR: tag $TAG already exists."
  exit 1
fi

# ── confirm ───────────────────────────────────────────────────────────────────
echo ""
echo "  Current : ${CURRENT}"
echo "  Next    : ${TAG}"
echo ""
read -rp "Create and push tag $TAG? [y/N] " CONFIRM
[[ "$CONFIRM" =~ ^[Yy]$ ]] || { echo "Aborted."; exit 0; }

# ── tag and push ──────────────────────────────────────────────────────────────
git tag "$TAG"
git push origin "$TAG"

echo ""
echo "✓ Tag $TAG pushed. GitHub Actions will build and publish the release."
echo "  https://github.com/meliani/Rustboard/releases/tag/$TAG"
