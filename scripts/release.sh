#!/usr/bin/env bash
# Publish Checklist

# Check if the VERSION environment variable is set
if [ -z "${VERSION}" ]; then
  echo "Error: VERSION environment variable is not set."
  echo "Usage: NAME=tod VERSION=0.6.15 ./release.sh"
  exit 1
fi

if [ -z "${NAME}" ]; then
  echo "Error: NAME environment variable is not set."
  echo "Usage: NAME=tod VERSION=0.6.15 ./release.sh"
  exit 1
fi

if [[ ! "${NAME}" =~ ^[a-zA-Z0-9_]+$ ]]; then
  echo "Error: NAME environment variable contains invalid characters. Only alphanumeric characters and underscores are allowed."
  exit 1
fi

cd ./target/release || { echo "Error: target/release directory does not exist. Ensure the build step completed successfully."; exit 1; }

echo "=== BUILDING RELEASE ===" &&
cargo build --release &&
echo "=== GZIPPING ===" &&
tar -czf "$NAME-mac.tar.gz" "$NAME"
cd ../..
echo "=== CREATING GITHUB RELEASE ===" &&
gh release create "v$VERSION" ./target/release/*.tar.gz --title "v$VERSION" --generate-notes &&
echo "=== RUNNING cargo publish FOR CRATES.IO ===" &&
cargo publish || { echo "Error: cargo publish failed. Please check your credentials, network connection, or other potential issues."; exit 1; }
# echo "=== RUNNING push_aur.sh TO PUSH NEW VERSION TO AUR ===" &&
# ./scripts/push_aur.sh &&
echo "=== DELETING MERGED BRANCHES ===" &&
git-delete-merged-branches --yes &&
echo "Hashing release" &&
if [ -f "./target/release/tod-mac.tar.gz" ]; then
  HASH=$(shasum -a 256 ./target/release/tod-mac.tar.gz | awk '{print $1}')
  echo "HASH:"
  echo $HASH
else
  echo "Error: File ./target/release/tod-mac.tar.gz does not exist. Ensure the tar command succeeded."
  exit 1
fi

cd ../homebrew-tod || { echo "Error: failed to change directory to ../homebrew-tod. Ensure the directory exists and is accessible."; exit 1; }
echo "Editing Homebrew versions to set version to $VERSION"
ambr --regex "version \"\\d+\\.\\d+\\.\\d+\"" "version \"$VERSION\"" Formula/tod.rb
ambr --regex "https://github.com/tod-org/tod/releases/download/v\d+\\.\\d+\\.\\d+/" "https://github.com/tod-org/tod/releases/download/v$VERSION/" Formula/tod.rb
echo "Editing Homebrew versions to set SHA256 to $HASH"
ambr --regex "sha256 \"[0-9a-z]+\"" "sha256 \"$HASH\"" Formula/tod.rb

if ! git fetch origin && git status | grep -q "Your branch is up to date with"; then
    echo "Error: The branch is not up-to-date with the remote. Please pull the latest changes before proceeding."
    exit 1
fi

git add . &&
git commit -m "$VERSION" &&
git push origin HEAD &&
echo "Homebrew update complete"
cd ../tod || exit
