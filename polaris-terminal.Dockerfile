# syntax=docker/dockerfile:1

FROM rust:1.87 AS builder
WORKDIR /usr/src/polaris

# Use the SSH agent to access private repositories
COPY root-config /root/
RUN sed 's|/home/runner|/root|g' -i.bak /root/.ssh/config

# Copy the Cargo.toml and Cargo.lock files first to leverage Docker cache
COPY Cargo.toml ./

# Copy the rest of the source code
COPY . .

# Pre-fetch crates (public + private)
RUN --mount=type=ssh cargo fetch

RUN apt-get update && apt-get install -y --no-install-recommends clang libclang-dev && rm -rf /var/lib/apt/lists/*

# ✱ SSH agent needed again here ✱
RUN --mount=type=ssh cargo build --release

FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    libssl3 ca-certificates libfontconfig && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/polaris/target/release/polaris-terminal /usr/local/bin/polaris-terminal

EXPOSE 8080

ENTRYPOINT [ "polaris-terminal" ]
