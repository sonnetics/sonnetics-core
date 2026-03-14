#!/usr/bin/env bash
# Publish sonnetics-core to npm (@sonnetics/core). Touches only core.
# Default: dry run (no commit, tag, or npm publish).
# Usage: ./publish-core.sh [--publish] [TAG]
#   --publish  Actually commit, tag, and publish to npm
#   TAG        npm tag, default: beta
# Example: ./publish-core.sh           # dry run
#          ./publish-core.sh --publish beta

set -e

PUBLISH=false
TAG="beta"
for arg in "$@"; do
  if [[ "$arg" == "--publish" ]]; then
    PUBLISH=true
  else
    TAG="$arg"
  fi
done

CORE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ ! -d "$CORE_DIR" ]]; then
  echo "Error: sonnetics-core not found at $CORE_DIR"
  exit 1
fi

if [[ "$PUBLISH" == true ]] && [[ -z "${SKIP_CLEAN_CHECK:-}" ]]; then
  if [[ -n $(git -C "$CORE_DIR" status --porcelain) ]]; then
    echo "Error: Uncommitted changes in sonnetics-core. Commit or stash first."
    exit 1
  fi
fi

cd "$CORE_DIR"

# Bump version from Cargo.toml: 0.0.1-beta.7 -> 0.0.1-beta.8
CURRENT=$(cargo pkgid 2>/dev/null | cut -d'#' -f2)
if [[ "$CURRENT" =~ ^(.*-beta\.)([0-9]+)$ ]]; then
  VERSION="${BASH_REMATCH[1]}$((${BASH_REMATCH[2]} + 1))"
else
  VERSION="${CURRENT}-beta.0"
fi

if npm view "@sonnetics/core@$VERSION" version &>/dev/null; then
  echo "Error: $VERSION already published on npm."
  exit 1
fi

if [[ "$PUBLISH" == true ]]; then
  echo "=== Publishing @sonnetics/core $VERSION (tag: $TAG) ==="
else
  echo "=== DRY RUN: @sonnetics/core $VERSION (tag: $TAG) ==="
fi

sed -i 's/^version = .*/version = "'"$VERSION"'"/' "$CORE_DIR/Cargo.toml"
sed -i 's/^version = .*/version = "'"$VERSION"'"/' "$CORE_DIR/pyproject.toml"

echo "Building WASM (pkg/)..."
wasm-pack build --target bundler --out-dir pkg

if [[ ! -f "$CORE_DIR/pkg/sonnetics_core_bg.wasm" ]]; then
  echo "Error: pkg/sonnetics_core_bg.wasm not found after build. Aborting."
  exit 1
fi

echo "Setting package name to @sonnetics/core..."
sed -i 's/"name": "sonnetics-core"/"name": "@sonnetics\/core"/' "$CORE_DIR/pkg/package.json"

if [[ "$PUBLISH" != true ]]; then
  echo ""
  echo "npm publish (dry run)..."
  cd "$CORE_DIR/pkg"
  npm publish --tag "$TAG" --access public --dry-run
  echo ""
  echo "Dry run complete. Run with --publish to commit, tag, and publish to npm:"
  echo "  ./publish-core.sh --publish $TAG"
  exit 0
fi

git add Cargo.toml pyproject.toml
git commit -m "chore: release $VERSION"

cd "$CORE_DIR/pkg"
npm publish --tag "$TAG" --access public
git -C "$CORE_DIR" tag "v$VERSION"

echo ""
echo "Done. Install with: npm install @sonnetics/core@$TAG"
