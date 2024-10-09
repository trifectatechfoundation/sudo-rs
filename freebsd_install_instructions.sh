pw usermod bjorn -G wheel
vim /usr/local/etc/sudoers # allow wheel to run any command without password
pkg install sudo vim git podman

zfs create -o mountpoint=/var/db/containers zroot/containers
cp /usr/local/etc/containers/pf.conf.sample /etc/pf.conf
vim /etc/pf.conf # adapt *egress_if
sysrc pf_enable=YES
service pf start
vim /usr/local/etc/containers/registries.conf # add unqualified-search-registries = ["docker.io"]

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
