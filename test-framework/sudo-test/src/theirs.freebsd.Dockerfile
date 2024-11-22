FROM dougrabson/freebsd14.1-small:latest
RUN IGNORE_OSVERSION=yes pkg install -y sudo pidof sshpass rsyslog bash vim dash FreeBSD-libbsm && \
    rm /usr/local/etc/sudoers
# just to match `ours.Dockerfile`
WORKDIR /tmp
