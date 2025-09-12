FROM debian:trixie-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends sudo procps sshpass rsyslog && \
    rm /etc/sudoers
# Ensure we use the same shell across OSes
RUN chsh -s /bin/sh
# To ensure we can create a user with uid 1000 and to avoid having to use uid 1001 in test expectations
RUN userdel ubuntu || true
# just to match `ours.Dockerfile`
WORKDIR /tmp
