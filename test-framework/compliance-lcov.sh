#!/bin/bash

set -euo pipefail

test "${1:-}" || (echo "use export, report or show" && false)

# This script requires LLVM-tools to be installed using:
#   rustup component add llvm-tools

# we need to call llvm-cov and llvm-profdata directly
PATH="$PATH:$(find "`rustc --print sysroot`" -executable -name 'llvm-*' -printf %h -quit)"

# find the project dir relative to the location of this script
project_dir="$(realpath "${0%/*}/..")"

profdata="$SUDO_TEST_PROFRAW_DIR"/sudo-rs.profdata
binary="$SUDO_TEST_PROFRAW_DIR"/sudo-rs

llvm-profdata merge \
	-sparse \
	"$SUDO_TEST_PROFRAW_DIR"/*/*.profraw \
	-o "$profdata"

dockerid=$(docker create sudo-test-rs)
docker cp -q "$dockerid":/usr/bin/sudo "$binary"
docker rm "$dockerid" > /dev/null

case "$1" in
export)
	llvm-cov export \
		-format=lcov \
		--ignore-filename-regex='.cargo/registry' \
		--ignore-filename-regex='/usr/local/cargo/registry' \
		--ignore-filename-regex='/rustc' \
		--instr-profile="$profdata" \
		--object "$binary" | sed "s:/usr/src/sudo:$project_dir:g";;

report)
	llvm-cov report \
		--color \
		--ignore-filename-regex='.cargo/registry' \
		--ignore-filename-regex='/usr/local/cargo/registry' \
		--ignore-filename-regex='/rustc' \
		--instr-profile="$profdata" \
		--object "$binary" \
		--object "$project_dir/target/debug/sudo" \
		-path-equivalence="/usr/src/sudo,$project_dir";;

show)
	llvm-cov show \
		--color \
		--ignore-filename-regex='.cargo/registry' \
		--ignore-filename-regex='/usr/local/cargo/registry' \
		--ignore-filename-regex='/rustc' \
		--instr-profile="$profdata" \
		--object "$binary" \
		-path-equivalence="/usr/src/sudo,$project_dir";;

*)
	echo "unknown llvm-cov command: use export, report or show"
	exit 1;;
esac
