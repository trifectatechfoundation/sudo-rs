FROM rust:1-slim-bookworm
RUN apt-get update && \
    apt-get install -y --no-install-recommends clang libclang-dev libpam0g-dev procps sshpass rsyslog
# cache the crates.io index in the image for faster local testing
RUN cargo search sudo
WORKDIR /usr/src/sudo
COPY . .
RUN --mount=type=cache,target=/usr/src/sudo/target cargo build --locked --features="dev" --bins && mkdir -p build && cp target/debug/sudo build/sudo && cp target/debug/su build/su && cp target/debug/visudo build/visudo
# set setuid on install
RUN install --mode 4755 build/sudo /usr/bin/sudo
RUN install --mode 4755 build/su /usr/bin/su
RUN install --mode 755 build/visudo /usr/sbin/visudo
# remove build dependencies
RUN apt-get autoremove -y clang libclang-dev
# set the default working directory to somewhere world writable so sudo / su can create .profraw files there
WORKDIR /tmp
