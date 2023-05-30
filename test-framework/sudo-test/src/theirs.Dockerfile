FROM debian:bullseye-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends sudo procps && \
    rm /etc/sudoers
