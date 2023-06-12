FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends sudo procps sshpass rsyslog && \
    rm /etc/sudoers
