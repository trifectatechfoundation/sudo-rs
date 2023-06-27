trap 'echo >&2 received SIGTERM; exit 22' TERM

for i in $(seq 1 7); do
	echo >&2 "$i"
	sleep 1
done
echo >&2 timeout
exit 11
