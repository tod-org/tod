#!/usr/bin/env bash

# This script automatically creates the commit to github that will initialize a release-please PR for release.
# It checks for required environment variables, ensures the working directory is clean, and creates a branch for the release.
# It also checks that the Cargo.toml version matches the VERSION environment variable and pushes to a branch.
# After execution, it opens a pull request with the appropriate label. The pull request will be labeled with "autorelease: pending".


# Check required environment variables
if [ -z "${VERSION:-}" ]; then
  echo "Error: VERSION environment variable is not set."
  echo "Usage: VERSION=0.6.15 ./release.sh"
  exit 1
fi

if [ -z "${NAME:-}" ]; then
  echo "Error: NAME environment variable is not set."
  echo "Usage: NAME=tod VERSION=0.6.15 ./release.sh"
  exit 1
fi

if [[ ! "$NAME" =~ ^[a-zA-Z0-9_]+$ ]]; then
  echo "Error: NAME contains invalid characters (only alphanumeric and underscores allowed)."
  exit 1
fi

# Ensure we are on the default branch
default_branch=$(git remote show origin | awk '/HEAD branch/ {print $NF}')
current_branch=$(git rev-parse --abbrev-ref HEAD)
if [[ "$current_branch" != "$default_branch" ]]; then
  echo "Error: You must be on the default branch ($default_branch) to run this script. You are on '$current_branch'."
  exit 1
fi

# Ensure working directory is clean
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Error: Working directory is not clean. Commit or stash your changes before continuing."
  exit 1
fi

# Check that Cargo.toml version matches
echo "==> Checking Cargo.toml version..."
cargo_version=$(grep -m1 '^version =' Cargo.toml | sed -E 's/version = "(.*)"/\1/')
if [[ "$cargo_version" != "$VERSION" ]]; then
  echo "Error: VERSION ($VERSION) does not match Cargo.toml version ($cargo_version)"
  exit 1
fi

# Create branch
branch="release-$VERSION"
echo "==> Creating branch $branch..."
git checkout -b "$branch"

# Optionally trigger release-please (empty commit or dummy file)
echo "==> Committing release prep..."
git commit --allow-empty -m "chore: release $VERSION"
git push origin "$branch"

# Create PR with label
echo "==> Opening pull request..."
gh pr create \
  --title "Release $VERSION" \
  --body "Prepare for release $VERSION" \
  --label "autorelease: pending" \
  --base "$default_branch" \
  --head "$branch"

echo "✅ Done."
