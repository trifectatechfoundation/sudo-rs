FROM rust:1.67
RUN apt-get update && \
    apt-get install -y --no-install-recommends libclang-dev libpam0g-dev
# cache the crates.io index in the image for faster local testing
RUN cargo search sudo
WORKDIR /usr/src/sudo
COPY . .
# TODO should use `--locked` but the repository does not include a `Cargo.lock` file
RUN cargo install --debug --path sudo
# HACK sudo-rs is hard-coded to use /etc/sudoers.test
RUN ln -s sudoers /etc/sudoers.test
