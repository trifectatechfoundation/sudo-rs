#!/usr/bin/env bash

exec docker run --rm -it -v "$(pwd):/data" -u "$(id -u):$(id -g)" "pandoc/core@sha256:668f5ced9d99ed0fd8b0efda93d6cead066565bb400fc1fb165e77ddbb586a16" "$@"
