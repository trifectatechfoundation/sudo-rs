set -e

date +%s%3N
(
	echo wrong-password
	echo strong-password
) | sudo -S date +%s%3N
