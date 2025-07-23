#!/bin/sh
until [ -f /tmp/barrier ]; do 
    sleep 0.1
done

target="$1"
shift

# when sudoedit runs there are two sudo processes as sudoedit spawns
# a child process. We assume the parent is the smallest of the returned PIDs.
case "$target" in
  child)
    sudopid=$(pidof sudoedit | tr ' ' '\n' | sort -gr | head -n1);;

  parent)
    sudopid=$(pidof sudoedit | tr ' ' '\n' | sort -g | head -n1);;

  editor)
    # same logic as before; but the most recent 'sh' instance will be
    # the subshell spawned, so we take the second-most-recent 'sh' to be
    # the editor; a better way would be to ps aux | grep, but this works
    # for now.
    sudopid=$(pidof sh | tr ' ' '\n' | sort -gr | sed -n 2p);;
esac

if [ -n "$sudopid" ]; then
    kill $1 "$sudopid"
fi
