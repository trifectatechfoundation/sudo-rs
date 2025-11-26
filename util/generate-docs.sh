#!/usr/bin/env bash

set -eo pipefail

docs_dir="docs/man"
files=("sudo.8" "visudo.8" "sudoers.5" "su.1")

function docker_pandoc() {
    docker run --rm -i -v "$(pwd):/data" -u "$(id -u):$(id -g)" "pandoc/core@sha256:668f5ced9d99ed0fd8b0efda93d6cead066565bb400fc1fb165e77ddbb586a16" "$@"
}

for f in "${files[@]}"; do
    origin_file="$docs_dir/$f.md"
    target_file="$docs_dir/$f.man"

    echo "Generating man page for $f from '$origin_file' to '$target_file'"
    docker_pandoc -s -t man "$origin_file" -o "$target_file"
done
