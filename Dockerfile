FROM rust:1-slim-bookworm
RUN apt-get update -y && apt-get install -y clang libclang-dev libpam0g-dev
