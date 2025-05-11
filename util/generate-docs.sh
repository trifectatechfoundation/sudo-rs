#!/usr/bin/env bash

docs_dir="docs/man"
files=("sudo.8" "visudo.8" "sudoers.5" "su.1")

for f in "${files[@]}"; do
    origin_file="$docs_dir/$f.md"
    target_file="$docs_dir/$f.man"

    echo "Generating man page for $f from '$origin_file' to '$target_file'"
    util/pandoc.sh -s -t man "$origin_file" -o "$target_file"
done
