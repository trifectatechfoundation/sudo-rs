# Save the name of the current tty so `change-size.sh` can read it.
tty > /tmp/tty_path 
# Print the current terminal size
stty size
# Print the terminal size, wait for `change-size.sh` to run and then print the terminal size again.
sudo sh -c "stty size; sleep 1; stty size"
