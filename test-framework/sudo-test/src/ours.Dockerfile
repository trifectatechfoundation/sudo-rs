FROM rust:1-slim-buster
RUN apt-get update && \
    apt-get install -y --no-install-recommends clang libclang-dev libpam0g-dev procps sshpass rsyslog
# cache the crates.io index in the image for faster local testing
RUN cargo search sudo
WORKDIR /usr/src/sudo
COPY . .
RUN --mount=type=cache,target=/usr/src/sudo/target RUSTFLAGS="-C instrument-coverage" cargo build --locked -p sudo && mkdir -p build && cp target/debug/sudo build/sudo
# discard code coverage data created during `cargo build`
RUN find / -name '*.profraw' -exec rm {} \;
# set setuid on install
RUN install --mode 4755 build/sudo /usr/bin/sudo
# remove build dependencies
RUN apt-get autoremove -y clang libclang-dev
# HACK sudo-rs is hard-coded to use /etc/sudoers.test
RUN ln -s sudoers /etc/sudoers.test
# Makes sure our sudo implementation actually runs
ENV SUDO_RS_IS_UNSTABLE="I accept that my system may break unexpectedly"
