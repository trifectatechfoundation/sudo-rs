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
    sudopid=$(ps ax | awk '/bin\/sh \/usr\/bin\/(editor|vi)/ {print $1}');;
esac

if [ -n "$sudopid" ]; then
    kill $1 "$sudopid"
fi
