# Build stage
FROM rust:1.80-slim-bookworm AS builder

WORKDIR /usr/src/app
COPY . .

# We need to install pkg-config and openssl for dependencies if required
RUN apt-get update && apt-get install -y pkg-config libssl-dev

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /usr/src/app
COPY --from=builder /usr/src/app/target/release/server /usr/src/app/server

# Install CA certificates for Turso/TLS connections
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

EXPOSE 8080

CMD ["./server"]
