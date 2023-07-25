visudopid=$(pidof visudo | sort -gr | cut -f 1 -d ' ')

if [ -n "$visudopid" ]; then
    kill $1 "$visudopid"
fi
