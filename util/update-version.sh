#!/usr/bin/env bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd)
PROJECT_DIR=$(dirname "$SCRIPT_DIR")

# Fetch current version
CURRENT_VERSION=$(sed -n '/^version\s*=\s*"\([0-9.]*\)"/{s//\1/p;q}' "$PROJECT_DIR/Cargo.toml")

# Fetch new version from changelog
NEW_VERSION=$(grep -m1 '^##' "$PROJECT_DIR"/CHANGELOG.md | grep -o "\[[0-9]\+.[0-9]\+.[0-9]\+\]" | tr -d '[]')

if [ -z "$NEW_VERSION" ]; then
    echo "Could not fetch version from CHANGELOG.md; you probably made a mistake."
    exit 1
fi

if ! grep -m1 '^## ' "$PROJECT_DIR"/CHANGELOG.md | grep -q -o "[0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}"; then
    echo "Date not formatted correctly in CHANGELOG.md."
    exit 1
fi

if ! grep -m2 '^## ' "$PROJECT_DIR"/CHANGELOG.md | grep -o "[0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}" | sort -r --check=silent; then
    echo "Release date of $NEW_VERSION must be more recent than the previous version."
    exit 1
fi

YEAR=$(grep -m1 '^## ' "$PROJECT_DIR"/CHANGELOG.md | grep -o "[0-9]\{4\}")
for license in COPYRIGHT LICENSE-MIT; do
    if ! grep -q "$YEAR" "$PROJECT_DIR/$license"; then
        echo "Bump year in $license to $YEAR"
        exit 1
    fi
done

if [ "$CURRENT_VERSION" == "$NEW_VERSION" ]; then
    echo "Cargo.toml was already at $NEW_VERSION"
    exit 2
else
    echo "Updating version in Cargo.toml to $NEW_VERSION"
    sed -i 's/^version\s*=\s*".*"/version = "'"$NEW_VERSION"'"/' "$PROJECT_DIR/Cargo.toml"
fi

echo "Updating version in README.md installation instructions"
sed -i 's/sudo-\(VERSION\|[0-9]\+\.[0-9]\+\.[0-9]\+\)\.tar\.gz/sudo-'"$NEW_VERSION"'\.tar\.gz/g' "$PROJECT_DIR/README.md"
sed -i 's/su-\(VERSION\|[0-9]\+\.[0-9]\+\.[0-9]\+\)\.tar\.gz/su-'"$NEW_VERSION"'\.tar\.gz/g' "$PROJECT_DIR/README.md"

echo "Updating version in man pages"
sed -i 's/^title: SU(1) sudo-rs .*/title: SU(1) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/su.1.md
sed -i 's/^title: SUDO(8) sudo-rs .*/title: SUDO(8) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/sudo.8.md
sed -i 's/^title: VISUDO(8) sudo-rs .*/title: VISUDO(8) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/visudo.8.md
sed -i 's/^title: SUDOERS(5) sudo-rs .*/title: SUDOERS(5) sudo-rs '"$NEW_VERSION"' | sudo-rs/' "$PROJECT_DIR"/docs/man/sudoers.5.md

echo "Regenerate man pages"
"$PROJECT_DIR/util/generate-docs.sh"

echo "Rebuilding project"
# NOTE: Not using --locked as Cargo.lock needs to be updated with the new version
(cd $PROJECT_DIR && cargo build --release)
