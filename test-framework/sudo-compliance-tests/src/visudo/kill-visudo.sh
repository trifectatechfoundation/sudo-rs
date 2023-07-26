visudopid=$(pidof visudo | sort -gr | cut -f 1 -d ' ')

if [ -n "$visudopid" ]; then
    sleep 1
    kill $1 "$visudopid"
fi
