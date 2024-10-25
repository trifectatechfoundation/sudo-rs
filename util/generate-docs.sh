#!/usr/bin/env bash

docs_dir="docs/man"
output_dir="target/docs/man"
files=("sudo.8" "visudo.8" "sudoers.5" "su.1")

mkdir -p "$output_dir"

for f in "${files[@]}"; do
    origin_file="$docs_dir/$f.md"
    target_file="$output_dir/$f"

    echo "Generating man page for $f from '$origin_file' to '$target_file'"
    util/pandoc.sh -s -t man "$origin_file" -o "$target_file"
done
