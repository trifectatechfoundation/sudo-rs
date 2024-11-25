FROM dougrabson/freebsd14.1-small:latest
RUN IGNORE_OSVERSION=yes pkg install -y sshpass rsyslog bash vim pidof dash
WORKDIR /usr/src/sudo
COPY target/build build
# set setuid on install
RUN install -m 4755 build/sudo /usr/bin/sudo && \
    install -m 4755 build/su /usr/bin/su && \
    install -m 755 build/visudo /usr/sbin/visudo
# `apt-get install sudo` creates this directory; creating it in the image saves us the work of creating it in each compliance test
RUN mkdir -p /usr/local/etc/sudoers.d
# Ensure we use the same shell across OSes
RUN chsh -s /bin/sh
# set the default working directory to somewhere world writable so sudo / su can create .profraw files there
WORKDIR /tmp
# Makes sure our sudo implementation actually runs
ENV SUDO_RS_IS_UNSTABLE="I accept that my system may break unexpectedly"
