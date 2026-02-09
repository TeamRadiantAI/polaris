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

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends chromium ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN update-ca-certificates

ENV CHROME_BIN=/usr/bin/chromium

COPY --from=builder /usr/src/polaris/target/release/polaris-server /usr/local/bin/polaris-server
COPY --from=builder /usr/src/polaris/target/release/polaris-observer /usr/local/bin/polaris-observer

EXPOSE 8080

ENTRYPOINT [ "polaris-server" ]
