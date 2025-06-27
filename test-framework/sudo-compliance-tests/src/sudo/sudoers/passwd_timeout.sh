#!/bin/sh

set -ex

tmp="$(mktemp)"
rm "${tmp}"
mkfifo "${tmp}"

# reads against the fifo will block without returning EOF
sudo -S true <> "${tmp}"
