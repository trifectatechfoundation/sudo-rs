FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends sudo procps sshpass rsyslog && \
    rm /etc/sudoers
# Ensure we use the same shell across OSes
RUN chsh -s /bin/sh
# just to match `ours.Dockerfile`
WORKDIR /tmp
