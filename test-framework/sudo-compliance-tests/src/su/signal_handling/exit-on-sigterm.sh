trap 'echo received SIGTERM; exit 22' TERM

for i in $(seq 1 7); do
	echo "$i"
	sleep 1
done
echo timeout
exit 11
