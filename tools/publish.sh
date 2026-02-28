#!/usr/bin/env bash
# crates.io publish script for Uika.
#
# Creates a temporary branch, applies crates.io-specific changes,
# publishes all crates in dependency order, then discards the branch.
#
# Usage:
#   ./tools/publish.sh          # dry-run (default)
#   ./tools/publish.sh --publish # actually publish to crates.io
#
# Prerequisites:
#   - cargo login (already authenticated)
#   - Clean working tree (no uncommitted changes)
#   - On the main branch

set -euo pipefail

DRY_RUN=true
if [[ "${1:-}" == "--publish" ]]; then
    DRY_RUN=false
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# --- Pre-flight checks ---

if [[ -n "$(git status --porcelain)" ]]; then
    echo "ERROR: Working tree is not clean. Commit or stash changes first."
    exit 1
fi

CURRENT_BRANCH="$(git branch --show-current)"
if [[ "$CURRENT_BRANCH" != "main" ]]; then
    echo "WARNING: Not on main branch (currently on '$CURRENT_BRANCH')."
    read -p "Continue anyway? [y/N] " -r
    [[ $REPLY =~ ^[Yy]$ ]] || exit 1
fi

echo "=== Uika crates.io publish ==="
if $DRY_RUN; then
    echo "    Mode: DRY RUN (use --publish for real)"
else
    echo "    Mode: PUBLISH (for real!)"
    read -p "Are you sure? [y/N] " -r
    [[ $REPLY =~ ^[Yy]$ ]] || exit 1
fi
echo ""

# --- Create temporary publish branch ---

PUBLISH_BRANCH="publish/$(date +%Y%m%d-%H%M%S)"
git checkout -b "$PUBLISH_BRANCH"

cleanup() {
    echo ""
    echo "=== Cleaning up ==="
    git checkout "$CURRENT_BRANCH"
    git branch -D "$PUBLISH_BRANCH"
    echo "Temporary branch '$PUBLISH_BRANCH' deleted."
}
trap cleanup EXIT

# --- Apply crates.io modifications ---

echo "--- Applying crates.io modifications ---"

# 1. Empty default features for uika and uika-bindings
sed -i 's/^default = \["core", "engine"\]/default = []/' uika/Cargo.toml
sed -i 's/^default = \["core"\]/default = []/' uika-bindings/Cargo.toml

echo "  [OK] Default features set to empty"

# 2. Sync ue_plugin_embed snapshot for uika-cli
echo "  Syncing ue_plugin_embed..."
rm -rf uika-cli/ue_plugin_embed/
mkdir -p uika-cli/ue_plugin_embed/

# Copy Uika plugin (exclude Generated, Binaries, Intermediate, obj, .props)
rsync -a \
    --exclude='Generated' \
    --exclude='Binaries' \
    --exclude='Intermediate' \
    --exclude='obj' \
    --exclude='*.csproj.props' \
    ue_plugin/Uika/ uika-cli/ue_plugin_embed/Uika/

# Copy UikaGenerator plugin
rsync -a \
    --exclude='Binaries' \
    --exclude='Intermediate' \
    --exclude='obj' \
    --exclude='*.csproj.props' \
    ue_plugin/UikaGenerator/ uika-cli/ue_plugin_embed/UikaGenerator/

echo "  [OK] ue_plugin_embed synced"

# 3. Commit temporary changes
git add -A
git commit -m "Temporary: prepare for crates.io publish"

echo ""

# --- Publish crates in dependency order ---

CRATES=(
    uika-ffi
    uika-macros
    uika-runtime
    uika-bindings
    uika-ue-flags
    uika-codegen
    uika
    uika-cli
)

PUBLISH_FLAG=""
if $DRY_RUN; then
    PUBLISH_FLAG="--dry-run"
fi

for crate in "${CRATES[@]}"; do
    echo "--- Publishing $crate $PUBLISH_FLAG ---"
    if cargo publish -p "$crate" $PUBLISH_FLAG --allow-dirty 2>&1; then
        echo "  [OK] $crate"
    else
        echo "  [FAIL] $crate — stopping."
        exit 1
    fi

    if ! $DRY_RUN; then
        echo "  Waiting 30s for crates.io index update..."
        sleep 30
    fi
    echo ""
done

echo "=== All crates published successfully ==="
if $DRY_RUN; then
    echo "(This was a dry run. Use --publish for real.)"
fi
