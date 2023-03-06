FROM rust:1.67
RUN apt-get update && \
    apt-get install -y --no-install-recommends libclang-dev libpam0g-dev
WORKDIR /usr/src/sudo
COPY . .
# TODO should use `--locked` but the repository does not include a `Cargo.lock` file
RUN cargo install --debug --path sudo
