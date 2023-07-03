#!/usr/bin/env bash

set -e
# enable 'job control' to make `fg` work
set -m

su -c "sh -c 'for i in "'$(seq 1 5)'"; do date +%s; sleep 1; done'" &
sleep 2
kill -TSTP $!
sleep 5
fg
