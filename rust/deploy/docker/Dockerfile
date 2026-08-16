# Multi-stage Dockerfile for dsh-rust production deployment.
#
# Build:  docker build -t dsh-rust:latest -f deploy/docker/Dockerfile .
# Run:    docker run -p 3000:3000 -e DEEPSEEK_API_KEY=sk-xxx dsh-rust:latest

# ── Stage 1: Build ──────────────────────────────────────
FROM rust:1.82-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY apps/ apps/
COPY benches/ benches/

# Build release binaries
RUN cargo build --release -p dsh-web -p dsh-headless && \
    strip target/release/dsh-web && \
    strip target/release/dsh

# ── Stage 2: Runtime ────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && \
    apt-get install -y ca-certificates curl && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd -r dsh && \
    useradd -r -g dsh -d /app -s /sbin/nologin dsh

WORKDIR /app

# Copy binaries
COPY --from=builder /app/target/release/dsh-web /usr/local/bin/
COPY --from=builder /app/target/release/dsh /usr/local/bin/

# Create data directory
RUN mkdir -p /data/sessions && chown dsh:dsh /data/sessions

# Switch to non-root user
USER dsh

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

EXPOSE 3000

# Default to web server
CMD ["dsh-web", "--port", "3000", "--host", "0.0.0.0"]
