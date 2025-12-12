# Multi-stage Dockerfile for Halvor Web Application
# Supports both development and production builds

# ============================================================================
# Stage 1: Build Rust Server
# ============================================================================
FROM rust:1.75-slim AS rust-builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo files first (for better caching)
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./

# Create a dummy src to build dependencies (for better layer caching)
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && \
    cargo build --release --bin halvor --features sqlite,web-server,cli || true

# Copy actual source code
COPY src ./src

# Build Rust server (release or debug based on BUILD_TYPE)
ARG BUILD_TYPE=release
RUN if [ "$BUILD_TYPE" = "release" ]; then \
        cargo build --release --bin halvor --features sqlite,web-server,cli; \
    else \
        cargo build --bin halvor --features sqlite,web-server,cli; \
    fi

# ============================================================================
# Stage 2: Build Svelte Frontend
# ============================================================================
FROM node:24-alpine AS svelte-builder

WORKDIR /app

# Copy package files from halvor-web directory
COPY halvor-web/package.json halvor-web/package-lock.json* ./
RUN npm ci

# Copy Svelte source from halvor-web directory
COPY halvor-web/ ./

# Build the Svelte app (production or development based on BUILD_TYPE)
ARG BUILD_TYPE=release
RUN if [ "$BUILD_TYPE" = "release" ]; then \
        npm run build -- --mode production; \
    else \
        npm run build; \
    fi

# ============================================================================
# Stage 3: Development Runtime (with hot reload)
# ============================================================================
FROM node:24-alpine AS development

WORKDIR /app

# Install development dependencies (Rust toolchain for potential rebuilds + curl for health checks)
RUN apk add --no-cache \
    cargo \
    rust \
    python3 \
    make \
    gcc \
    musl-dev \
    ca-certificates \
    curl

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy Rust binary from builder
ARG BUILD_TYPE=release
COPY --from=rust-builder /build/target/${BUILD_TYPE}/halvor /usr/local/bin/halvor

# Copy Svelte source (for hot reload)
COPY halvor-web/package.json halvor-web/package-lock.json* ./
RUN npm install

COPY halvor-web/ ./

# Expose ports (dev server + web server)
EXPOSE 13001 13000

# Set environment variables
ENV RUST_LOG=info
ENV HALVOR_WEB_DIR=/app

# Health check script for development
RUN echo '#!/bin/sh\n\
curl -f http://localhost:13000/api/health || exit 1' > /app/healthcheck.sh && \
    chmod +x /app/healthcheck.sh

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["/app/healthcheck.sh"]

# Start dev server with watch mode
CMD ["npm", "run", "dev", "--", "--host", "0.0.0.0"]

# ============================================================================
# Stage 4: Production Runtime
# ============================================================================
FROM debian:bookworm-slim AS production

WORKDIR /app

# Install runtime dependencies (including curl for health checks)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Health check script
RUN echo '#!/bin/sh\n\
curl -f http://localhost:13000/api/health || exit 1' > /app/healthcheck.sh && \
    chmod +x /app/healthcheck.sh

# Copy Rust binary from builder
ARG BUILD_TYPE=release
COPY --from=rust-builder /build/target/${BUILD_TYPE}/halvor /usr/local/bin/halvor

# Copy Svelte build from builder
COPY --from=svelte-builder /app/build ./halvor-web/build

# Create directory for SQLite database (will be mounted from host)
RUN mkdir -p /app/data

# Expose web server port (13000)
EXPOSE 13000

# Set environment variables
ENV RUST_LOG=info
ENV HALVOR_WEB_DIR=/app/halvor-web
ENV HALVOR_DB_DIR=/app/data

# Health check script
RUN echo '#!/bin/sh\n\
curl -f http://localhost:13000/api/health || exit 1' > /app/healthcheck.sh && \
    chmod +x /app/healthcheck.sh

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD ["/app/healthcheck.sh"]

# Run the server (agent on 13001, web on 13000)
CMD ["halvor", "agent", "start", "--port", "13001"]
