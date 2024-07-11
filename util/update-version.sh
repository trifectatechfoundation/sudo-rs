#!/usr/bin/env bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd)
PROJECT_DIR=$(dirname "$SCRIPT_DIR")
NEW_VERSION="$1"

# Fetch current version
CURRENT_VERSION=$(sed -n '/version\s*=\s*"\([^"]*\)"/{s//\1/p;q}' "$PROJECT_DIR/Cargo.toml")

# Fetch new version from changelog
NEW_VERSION=$(grep -m1 '^##' "$PROJECT_DIR"/CHANGELOG.md | grep -o "\[[0-9]\+.[0-9]\+.[0-9]\+\]" | tr -d '[]')

if [ -z "$NEW_VERSION" ]; then
    echo "Could not fetch version from CHANGELOG.md; you probably made a mistake."
    exit 1
fi

if [ "$CURRENT_VERSION" \> "$NEW_VERSION" ]; then
    echo "New version number must be higher than current version: $CURRENT_VERSION"
    echo "Create a new changelog entry before running this script!"
    exit 1
fi

if [ "$CURRENT_VERSION" == "$NEW_VERSION" ]; then
    echo "Cargo.toml was already at $NEW_VERSION"
else
    echo "Updating version in Cargo.toml to $NEW_VERSION"
    sed -i 's/^version\s*=\s*".*"/version = "'"$NEW_VERSION"'"/' "$PROJECT_DIR/Cargo.toml"
fi

echo "Updating version in man pages"
sed -i 's/^title: SU(1) sudo-rs .*/title: SU(1) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/su.1.md
sed -i 's/^title: SUDO(8) sudo-rs .*/title: SUDO(8) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/sudo.8.md
sed -i 's/^title: VISUDO(8) sudo-rs .*/title: VISUDO(8) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/visudo.8.md

echo "Rebuilding project"
(cd $PROJECT_DIR && cargo build --release)
