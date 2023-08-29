#!/usr/bin/env bash

exec docker run --rm -it -v "$(pwd):/data" -u "$(id -u):$(id -g)" pandoc/core "$@"
