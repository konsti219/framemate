#!/usr/bin/env bash
# Sets the version everywhere, commits, tags `v<version>` and pushes both. The tag push
# triggers CI (.github/workflows/build.yml), which builds and publishes the GitHub release.
#
#   scripts/release.sh 0.2.0
set -euo pipefail

VERSION="${1:-}"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 <major.minor.patch>" >&2
  exit 2
fi
TAG="v$VERSION"

cd "$(dirname "$0")/.."
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "Working tree has uncommitted changes; commit or stash them first." >&2
  exit 1
fi
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "Tag $TAG already exists." >&2
  exit 1
fi

# Agent (workspace) and app shell versions; the APK's version comes from tauri.conf.json.
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml app/src-tauri/Cargo.toml
sed -i "0,/\"version\": \".*\"/s//\"version\": \"$VERSION\"/" app/src-tauri/tauri.conf.json app/package.json
# Refresh our own entries in both lockfiles (no dependency changes).
cargo update --workspace --offline --quiet
(cd app/src-tauri && cargo update --workspace --offline --quiet)

git add Cargo.toml Cargo.lock app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock \
  app/src-tauri/tauri.conf.json app/package.json
git commit -q -m "Release $TAG"
git tag -a "$TAG" -m "FrameMate $TAG"
git push --atomic origin HEAD "$TAG"
echo "Pushed $TAG; CI builds and publishes the release."
