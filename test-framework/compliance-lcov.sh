#!/bin/bash

set -euo pipefail

test "${1:-}" || (echo "use export, report or show" && false)

# This script requires LLVM-tools to be installed using:
#   rustup component add llvm-tools

# we need to call llvm-cov and llvm-profdata directly
PATH="$(rustc --print sysroot)/lib/rustlib/$(rustc --print host-tuple)/bin:$PATH"

# find the project dir relative to the location of this script
project_dir="$(realpath "${0%/*}/..")"

profdata="$SUDO_TEST_PROFRAW_DIR"/sudo-rs.profdata
binary="$SUDO_TEST_PROFRAW_DIR"/sudo-rs

# to merge coverage data from unit tests is only possible with 'export'
# also, the function coverage will become flawed

llvm-profdata merge \
	-sparse \
	"$SUDO_TEST_PROFRAW_DIR"/*/*.profraw \
	$(find "$project_dir"/target/llvm-cov-target -maxdepth 1 -type f -name "*.profraw" -print 2> /dev/null) \
	-o "$profdata"

dockerid=$(docker create sudo-test-rs)
docker cp -q "$dockerid":/usr/bin/sudo "$binary"
docker rm "$dockerid" > /dev/null

arg="$1"
shift

case "$arg" in
export)
	llvm-cov export \
		-format=lcov \
		--ignore-filename-regex='[.]cargo/registry' \
		--ignore-filename-regex='/usr/local/cargo/registry' \
		--ignore-filename-regex='/rustc' \
		--ignore-filename-regex='src/bin' \
		--ignore-filename-regex='sudoers/test' \
		--ignore-filename-regex='tests[.]rs' \
		--ignore-filename-regex='gettext/check[.]rs' \
		--instr-profile="$profdata" \
		--object "$binary" \
                $(find "$project_dir"/target/llvm-cov-target/debug/deps -maxdepth 1 -type f -executable -printf '--object %p\n') \
                "$@" \
                | sed "s:/usr/src/sudo:$project_dir:g";;

report)
	llvm-cov report \
		--color \
		--ignore-filename-regex='[.]cargo/registry' \
		--ignore-filename-regex='/usr/local/cargo/registry' \
		--ignore-filename-regex='/rustc' \
		--ignore-filename-regex='src/bin' \
		--ignore-filename-regex='sudoers/test' \
		--ignore-filename-regex='tests[.]rs' \
		--ignore-filename-regex='gettext/check[.]rs' \
		--instr-profile="$profdata" \
		--object "$binary" \
		-path-equivalence="/usr/src/sudo,$project_dir" \
                "$@";;

show)
	llvm-cov show \
		--color \
		--ignore-filename-regex='[.]cargo/registry' \
		--ignore-filename-regex='/usr/local/cargo/registry' \
		--ignore-filename-regex='/rustc' \
		--ignore-filename-regex='src/bin' \
		--ignore-filename-regex='sudoers/test' \
		--ignore-filename-regex='tests[.]rs' \
		--ignore-filename-regex='gettext/check[.]rs' \
		--instr-profile="$profdata" \
		--object "$binary" \
		-path-equivalence="/usr/src/sudo,$project_dir" \
                "$@";;

*)
	echo "unknown llvm-cov command: use export, report or show"
	exit 1;;
esac
