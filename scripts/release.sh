#!/bin/bash

# Release script for proxyctl-rs
# Usage: ./scripts/release.sh <version>

set -e

if [ $# -ne 1 ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 1.2.3"
    exit 1
fi

VERSION=$1
TAG="v$VERSION"

echo "Creating release $TAG"

# Update version in Cargo.toml
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Commit version bump
git add Cargo.toml Cargo.lock
git commit -m "Bump version to $VERSION"

# Create and push tag
git tag "$TAG"
git push origin main
git push origin "$TAG"

echo "Release $TAG created!"
echo "GitHub Actions will build and publish the release automatically."