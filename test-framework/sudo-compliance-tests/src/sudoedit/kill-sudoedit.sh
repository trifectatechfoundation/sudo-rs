until [ -f /tmp/barrier ]; do 
    sleep 0.1
done

target="$1"
shift

# when sudoedit runs there are two sudo processes as sudoedit spawns
# a child process. We assume the parent is the smallest of the returned PIDs.
if [ "$target" == "child" ]; then
    sudopid=$(pidof sudoedit | sort -g | cut -f 1 -d ' ')
else
    sudopid=$(pidof sudoedit | sort -gr | cut -f 1 -d ' ')
fi

if [ -n "$sudopid" ]; then
    kill $1 "$sudopid"
fi
