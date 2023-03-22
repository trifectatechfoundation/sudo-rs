# because the sudo process is `spawn`-ed it may not be immediately visible so
# retry `pidof` until it becomes visible
for _ in $(seq 1 20); do
	sudopid="$(pidof sudo)"
	if [ -n "$sudopid" ]; then
		# give `expects-signal.sh ` some time to execute the `trap` command otherwise
		# it'll be terminated before the signal handler is installed
		sleep 0.1
		kill "$sudopid"
		exit 0
	fi
	sleep 0.1
done

echo >&2 timeout
exit 1
