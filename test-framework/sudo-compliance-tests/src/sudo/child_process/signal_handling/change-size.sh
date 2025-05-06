# Wait for `print-sizes.sh` to write to `/tmp/tty_path` and report the old tty size.
until [ -f /tmp/barrier1 ]; do sleep 0.1; done
# Resize the terminal
stty -F$(cat /tmp/tty_path) rows 42 cols 69
# Notify `print-sizes.sh` that the tty size has changed.
touch /tmp/barrier2
