FROM dougrabson/freebsd14.1-small:latest
RUN IGNORE_OSVERSION=yes pkg install -y sudo pidof sshpass rsyslog bash vim dash FreeBSD-libbsm socat && \
    rm /usr/local/etc/sudoers
# Ensure we use the same shell across OSes
RUN chsh -s /bin/sh
# just to match `ours.Dockerfile`
WORKDIR /tmp
