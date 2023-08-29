#!/usr/bin/env bash

if [ "$#" -lt 1 ]; then
    echo "Missing new version"
    exit 1
fi

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd)
PROJECT_DIR=$(dirname "$SCRIPT_DIR")
NEW_VERSION="$1"

echo "Updating version in Cargo.toml"
sed -i 's/^version\s*=\s*".*"/version = "'"$NEW_VERSION"'"/' "$PROJECT_DIR/Cargo.toml"

echo "Updating version in man pages"
sed -i 's/^title: SU(1) sudo-rs .*/title: SU(1) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/su.1.md
sed -i 's/^title: SUDO(8) sudo-rs .*/title: SUDO(8) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/sudo.8.md
sed -i 's/^title: VISUDO(8) sudo-rs .*/title: VISUDO(8) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/visudo.8.md

echo "Rebuilding project"
(cd $PROJECT_DIR && cargo build --release)

echo "Version changes complete, you must still fill in the changelog entries"
