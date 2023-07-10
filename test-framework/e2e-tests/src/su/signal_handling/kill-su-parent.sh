#!/usr/bin/env sh
#
trap 'echo received SIGTERM; exit 11' TERM
su_pid="$(pidof su)"
[ -n "$su_pid" ] || (echo su process not found && exit 22)
kill "$su_pid"

# as insurance, wait a bit for signal delivery to happen
# the signal handler won't run while `sleep` runs hence the multiple `sleep` invocations
for _ in $(seq 1 10); do
    sleep 0.1
done
