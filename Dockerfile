# Build stage
FROM rust:1.86-slim-bullseye as builder

WORKDIR /usr/src/app

# Install dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl-dev pkg-config libsqlite3-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Install sqlx-cli for migrations
RUN cargo install sqlx-cli --no-default-features --features native-tls,sqlite

# Copy over manifests and lock files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src/ ./src/
COPY .sqlx/ ./.sqlx/
COPY migrations/ ./migrations/
COPY scripts/ ./scripts/

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl1.1 libsqlite3-0 ca-certificates strace && \
    rm -rf /var/lib/apt/lists/*

# Copy the built binary
COPY --from=builder /usr/src/app/target/release/kittyscape-loot-bot /app/
COPY --from=builder /usr/src/app/migrations/ /app/migrations/

# Set up the entrypoint
CMD ["/app/kittyscape-loot-bot"] 