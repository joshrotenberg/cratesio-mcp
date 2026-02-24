# Build stage
FROM rust:bookworm AS builder

WORKDIR /build

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source file to build dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > src/lib.rs

# Build dependencies only
RUN cargo build --release

# Remove dummy source files
RUN rm -rf src

# Copy actual source code
COPY src src

# Touch files to ensure rebuild
RUN touch src/main.rs src/lib.rs

# Build the actual binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies and create non-root user
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 cratesio-mcp

# Copy binary from builder
COPY --from=builder /build/target/release/cratesio-mcp /usr/local/bin/cratesio-mcp

# Switch to non-root user
USER cratesio-mcp

EXPOSE 3000

ENTRYPOINT ["cratesio-mcp"]
CMD ["--transport", "http", "--host", "0.0.0.0"]
