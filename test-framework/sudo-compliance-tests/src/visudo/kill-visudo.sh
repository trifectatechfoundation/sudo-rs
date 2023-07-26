visudopid=$(pidof visudo | sort -gr | cut -f 1 -d ' ')

if [ -n "$visudopid" ]; then
    until [ -f /tmp/barrier ]; do 
        sleep 0.1
    done
    kill $1 "$visudopid"
fi
