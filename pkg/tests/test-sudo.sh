#!/usr/bin/env bash

set -eo pipefail
set -x

case $1 in
    post-install|post-upgrade)
        [[ $(find /usr/bin/sudo -perm -g=s -exec echo SUDO-RS-HAS-SETUID \;) == "SUDO-RS-HAS-SETUID" ]]
    ;;
esac
