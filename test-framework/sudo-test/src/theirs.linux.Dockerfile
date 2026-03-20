FROM debian:trixie-slim@sha256:1d3c811171a08a5adaa4a163fbafd96b61b87aa871bbc7aa15431ac275d3d430
RUN apt-get update && \
    apt-get install -y --no-install-recommends sudo procps sshpass rsyslog socat acl && \
    rm /etc/sudoers
# Ensure we use the same shell across OSes
RUN chsh -s /bin/sh
# To ensure we can create a user with uid 1000 and to avoid having to use uid 1001 in test expectations
RUN userdel ubuntu || true
# just to match `ours.Dockerfile`
WORKDIR /tmp
