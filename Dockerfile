# Multi-stage build for optimal image size
FROM rustlang/rust:nightly-slim AS builder

WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY examples ./examples

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Build release binary
RUN cargo build --release && \
    strip target/release/pollinet_knowledge_bot

# Runtime stage - minimal image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder stage
COPY --from=builder /app/target/release/pollinet_knowledge_bot /usr/local/bin/

# Create non-root user for security
RUN useradd -m -u 1001 botuser
USER botuser

# Run the bot
CMD ["pollinet_knowledge_bot"]

