# because the sudo process is `spawn`-ed it may not be immediately visible so
# retry `pidof` until it becomes visible
for _ in $(seq 1 20); do
	sudopid="$(pidof sudo)"
	if [ -n "$sudopid" ]; then
		kill "$sudopid"
		exit 0
	fi
	sleep 0.1
done

echo >&2 timeout
exit 1
