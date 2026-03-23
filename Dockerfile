# syntax=docker/dockerfile:1

# Base image shared by planner and builder
FROM rust:1.94-slim-bookworm AS base

WORKDIR /usr/src/app

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libsqlite3-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

# Planner stage: build dependency recipe
FROM base AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage: cache dependencies, then compile app
FROM base AS builder

COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release --bin kittyscape-loot-bot

# Runtime stage
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl3 libsqlite3-0 ca-certificates strace && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/kittyscape-loot-bot /app/kittyscape-loot-bot
COPY --from=builder /usr/src/app/migrations/ /app/migrations/

CMD ["/app/kittyscape-loot-bot"]
