# Save the name of the current tty so `change-size.sh` can read it.
tty > /tmp/tty_path
# Print the current terminal size
stty size
# Print the terminal size, notify `change-size.sh` that it can change the tty size, wait for it to
# finish and then print the terminal size again.
sudo sh -c "stty size; touch /tmp/barrier1; until [ -f /tmp/barrier2 ]; do sleep 0.1; done; stty size"
