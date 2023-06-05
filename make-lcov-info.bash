#!/bin/bash

set -euo pipefail

rustup component add llvm-tools

llvm_profdata=$(find "$(rustc --print sysroot)" -name llvm-profdata)
profdata="$SUDO_TEST_PROFRAW_DIR"/sudo-rs.profdata
$llvm_profdata merge \
	-sparse \
	"$SUDO_TEST_PROFRAW_DIR"/**/*.profraw \
	-o "$profdata"

binary="$SUDO_TEST_PROFRAW_DIR"/sudo-rs
dockerid=$(docker create sudo-test-rs)
docker cp "$dockerid":/usr/bin/sudo "$binary"
docker rm "$dockerid"

llvm_cov="$(dirname "$llvm_profdata")"/llvm-cov
$llvm_cov export \
	-format=lcov \
	--ignore-filename-regex='/usr/local/cargo/registry' \
	--ignore-filename-regex='/rustc' \
	--instr-profile="$profdata" \
	--object "$binary" \
	-path-equivalence=/usr/src/sudo,"$(pwd)" >lcov.info
