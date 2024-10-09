FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends procps sshpass rsyslog
WORKDIR /usr/src/sudo
COPY target/build build
# set setuid on install
RUN install --mode 4755 build/sudo /usr/bin/sudo
RUN install --mode 4755 build/su /usr/bin/su
RUN install --mode 755 build/visudo /usr/sbin/visudo
# `apt-get install sudo` creates this directory; creating it in the image saves us the work of creating it in each compliance test
RUN mkdir -p /etc/sudoers.d
# set the default working directory to somewhere world writable so sudo / su can create .profraw files there
WORKDIR /tmp
