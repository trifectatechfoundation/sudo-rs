# Wait for `print-sizes.sh` to write to `/tmp/tty_path`
sleep 0.5
# Resize the terminal
stty -F$(cat /tmp/tty_path) rows 42 cols 69
