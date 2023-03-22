trap 'echo received SIGTERM; exit 1' TERM
sudo_pid="$(pidof sudo)"
[ -n "$sudo_pid" ] || (echo sudo process not found && exit 2)
kill "$sudo_pid"
# as insurance, wait a bit for signal delivery to happen
# the signal handler won't run while `sleep` runs hence the multiple `sleep` invocations
for _ in $(seq 1 10); do
	sleep 0.1
done
