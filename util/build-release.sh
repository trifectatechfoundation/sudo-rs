#!/usr/bin/env bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd)
PROJECT_DIR=$(dirname "$SCRIPT_DIR")
SUDO_RS_VERSION="$(cargo metadata --format-version 1 --manifest-path "$PROJECT_DIR/Cargo.toml" | jq '.packages[] | select(.name=="sudo-rs") | .version' -r)"
BUILDER_IMAGE_TAG="sudo-rs-release-builder:latest"
TARGET_DIR_BASE="$PROJECT_DIR/target/pkg"

set -eo pipefail

# Fetch the date from the changelog
DATE=$(grep -m1 '^##' "$PROJECT_DIR"/CHANGELOG.md | grep -o '[0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}')

# Build binaries
docker build --pull --tag "$BUILDER_IMAGE_TAG" --file "$SCRIPT_DIR/Dockerfile-release" "$SCRIPT_DIR"
docker run --rm --user "$(id -u):$(id -g)" -v "$PROJECT_DIR:/build" -w "/build" "$BUILDER_IMAGE_TAG" cargo clean
docker run --rm --user "$(id -u):$(id -g)" -v "$PROJECT_DIR:/build" -w "/build" "$BUILDER_IMAGE_TAG" cargo build --release --features pam-login,apparmor

# Generate man pages
"$PROJECT_DIR/util/generate-docs.sh"

# Set target directories and clear any previous builds
target_dir_sudo="$TARGET_DIR_BASE/sudo"
target_dir_su="$TARGET_DIR_BASE/su"
target_sudo="$TARGET_DIR_BASE/sudo-$SUDO_RS_VERSION.tar.gz"
target_su="$TARGET_DIR_BASE/su-$SUDO_RS_VERSION.tar.gz"

rm -rf "$target_dir_sudo"
rm -rf "$target_dir_su"
rm -rf "$target_su"
rm -rf "$target_sudo"

# Show what is happening
set -x

# Build sudo
umask u=rwx,g=rx,o=rx
mkdir -p "$target_dir_sudo/bin"
mkdir -p "$target_dir_sudo/share/man/man8"
mkdir -p "$target_dir_sudo/share/man/man5"
cp "$PROJECT_DIR/target/release/sudo" "$target_dir_sudo/bin/sudo"
cp "$PROJECT_DIR/target/release/visudo" "$target_dir_sudo/bin/visudo"
cp "$PROJECT_DIR/docs/man/sudo.8.man" "$target_dir_sudo/share/man/man8/sudo.8"
cp "$PROJECT_DIR/docs/man/visudo.8.man" "$target_dir_sudo/share/man/man8/visudo.8"
cp "$PROJECT_DIR/docs/man/sudoers.5.man" "$target_dir_sudo/share/man/man5/sudoers.5"
mkdir -p "$target_dir_sudo/share/doc/sudo-rs/sudo"
cp "$PROJECT_DIR/README.md" "$target_dir_sudo/share/doc/sudo-rs/sudo/README.md"
cp "$PROJECT_DIR/CHANGELOG.md" "$target_dir_sudo/share/doc/sudo-rs/sudo/CHANGELOG.md"
cp "$PROJECT_DIR/SECURITY.md" "$target_dir_sudo/share/doc/sudo-rs/sudo/SECURITY.md"
cp "$PROJECT_DIR/COPYRIGHT" "$target_dir_sudo/share/doc/sudo-rs/sudo/COPYRIGHT"
cp "$PROJECT_DIR/LICENSE-APACHE" "$target_dir_sudo/share/doc/sudo-rs/sudo/LICENSE-APACHE"
cp "$PROJECT_DIR/LICENSE-MIT" "$target_dir_sudo/share/doc/sudo-rs/sudo/LICENSE-MIT"

fakeroot -- bash <<EOF
set -eo pipefail
set -x
chown -R root:root "$target_dir_sudo"
chmod +xs "$target_dir_sudo/bin/sudo"
chmod +x "$target_dir_sudo/bin/visudo"
(cd $target_dir_sudo && tar --mtime="UTC $DATE 00:00:00" --sort=name --use-compress-program='gzip -9n' -cpvf "$target_sudo" *)
EOF

# Build su
mkdir -p "$target_dir_su/bin"
mkdir -p "$target_dir_su/share/man/man1"
cp "$PROJECT_DIR/target/release/su" "$target_dir_su/bin/su"
cp "$PROJECT_DIR/target/docs/man/su.1" "$target_dir_su/share/man/man1/su.1"
mkdir -p "$target_dir_su/share/doc/sudo-rs/su"
cp "$PROJECT_DIR/README.md" "$target_dir_su/share/doc/sudo-rs/su/README.md"
cp "$PROJECT_DIR/CHANGELOG.md" "$target_dir_su/share/doc/sudo-rs/su/CHANGELOG.md"
cp "$PROJECT_DIR/SECURITY.md" "$target_dir_su/share/doc/sudo-rs/su/SECURITY.md"
cp "$PROJECT_DIR/COPYRIGHT" "$target_dir_su/share/doc/sudo-rs/su/COPYRIGHT"
cp "$PROJECT_DIR/LICENSE-APACHE" "$target_dir_su/share/doc/sudo-rs/su/LICENSE-APACHE"
cp "$PROJECT_DIR/LICENSE-MIT" "$target_dir_su/share/doc/sudo-rs/su/LICENSE-MIT"

fakeroot -- bash <<EOF
set -eo pipefail
set -x
chown -R root:root "$target_dir_su"
chmod +xs "$target_dir_su/bin/su"
(cd $target_dir_su && tar --mtime="UTC $DATE 00:00:00" --sort=name --use-compress-program='gzip -9n' -cpvf "$target_su" *)
EOF

(cd $TARGET_DIR_BASE && sha256sum -b *-$SUDO_RS_VERSION.tar.gz > "$TARGET_DIR_BASE/SHA256SUMS")
