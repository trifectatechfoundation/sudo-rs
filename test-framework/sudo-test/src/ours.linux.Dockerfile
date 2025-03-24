FROM rust:1-slim-bookworm
RUN apt-get update && \
    apt-get install -y --no-install-recommends clang libclang-dev libpam0g-dev procps sshpass rsyslog
# cache the crates.io index in the image for faster local testing
RUN cargo search sudo
WORKDIR /usr/src/sudo
COPY . .
RUN --mount=type=cache,target=/usr/src/sudo/target cargo build --locked --features="pam-login" --bins && mkdir -p build && cp target/debug/sudo build/sudo && cp target/debug/su build/su && cp target/debug/visudo build/visudo
# set setuid on install
RUN install -m 4755 build/sudo /usr/bin/sudo && \
    install -m 4755 build/su /usr/bin/su && \
    install -m 755 build/visudo /usr/sbin/visudo
# `apt-get install sudo` creates this directory; creating it in the image saves us the work of creating it in each compliance test
RUN mkdir -p /etc/sudoers.d
# Ensure we use the same shell across OSes
RUN chsh -s /bin/sh
# remove build dependencies
RUN apt-get autoremove -y clang libclang-dev
# set the default working directory to somewhere world writable so sudo / su can create .profraw files there
WORKDIR /tmp
# This env var needs to be set when compiled with the dev feature
#ENV SUDO_RS_IS_UNSTABLE="I accept that my system may break unexpectedly"
