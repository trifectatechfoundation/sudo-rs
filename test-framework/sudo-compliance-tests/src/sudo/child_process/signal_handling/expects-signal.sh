trap 'echo got signal && exit 0' $@
for _ in $(seq 1 20); do
	sleep 0.1
done
echo >&2 received no signal
exit 1
