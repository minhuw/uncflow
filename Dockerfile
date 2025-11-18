# Multi-stage build for uncflow
# Stage 1: Build the Rust binary
FROM rust:1.83-slim AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy the entire project
COPY . .

# Build the release binary
RUN cargo build --release

# Stage 2: Runtime image
FROM ubuntu:24.04

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary from builder stage
COPY --from=builder /build/target/release/uncflow /usr/local/bin/uncflow

# Set environment variable to indicate Docker environment
ENV DOCKER_RUNNING=true

# Expose Prometheus metrics port
EXPOSE 8080

# Run uncflow with default metrics (IIO, IMC, IRP)
# Users can override this by passing different flags
ENTRYPOINT ["/usr/local/bin/uncflow"]
