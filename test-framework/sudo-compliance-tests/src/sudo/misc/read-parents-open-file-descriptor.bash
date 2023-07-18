echo topsecret >/tmp/secret.txt
exec 42<>/tmp/secret.txt

sudo bash -c 'cat <&42'
