#!/usr/bin/env bash

docs_dir="docs/man"
output_dir="target/docs/man"
files=("sudo.8" "visudo.8" "su.1")

mkdir -p "$output_dir"

for f in "${files[@]}"; do
    origin_file="$docs_dir/$f.md"
    tmp_file="$output_dir/$f.md"
    target_file="$output_dir/$f"

    echo "Generating man page for $f from '$origin_file' to '$target_file'"
    sed '/<!-- ---/s/<!-- ---/---/' "$origin_file" | sed '/--- -->/s/--- -->/---/' > "$tmp_file"
    util/pandoc.sh -s -t man "$tmp_file" -o "$target_file"
    rm "$tmp_file"
done
