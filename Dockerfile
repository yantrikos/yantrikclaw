# syntax=docker/dockerfile:1.7

# ── Stage 0: Frontend build ─────────────────────────────────────
FROM node:22-alpine AS web-builder
WORKDIR /web
COPY web/package.json web/package-lock.json* ./
RUN npm ci --ignore-scripts 2>/dev/null || npm install --ignore-scripts
COPY web/ .
RUN npm run build

# ── Stage 1: Build ────────────────────────────────────────────
FROM rust:1.91-slim AS builder

WORKDIR /app
ARG YANTRIKCLAW_CARGO_FEATURES="memory-postgres"

# Install build dependencies
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

# 1. Copy manifests to cache dependencies
COPY Cargo.toml Cargo.lock ./
# Remove robot-kit from workspace members — it is excluded by .dockerignore
# and is not needed for the Docker build (hardware-only crate).
RUN sed -i 's/members = \[".", "crates\/robot-kit"\]/members = ["."]/' Cargo.toml
# Create dummy targets declared in Cargo.toml so manifest parsing succeeds.
RUN mkdir -p src benches \
    && echo "fn main() {}" > src/main.rs \
    && echo "" > src/lib.rs \
    && echo "fn main() {}" > benches/agent_benchmarks.rs
RUN --mount=type=cache,id=yantrikclaw-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=yantrikclaw-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=yantrikclaw-target,target=/app/target,sharing=locked \
    if [ -n "$YANTRIKCLAW_CARGO_FEATURES" ]; then \
      cargo build --release --locked --features "$YANTRIKCLAW_CARGO_FEATURES"; \
    else \
      cargo build --release --locked; \
    fi
RUN rm -rf src benches

# 2. Copy only build-relevant source paths (avoid cache-busting on docs/tests/scripts)
COPY src/ src/
COPY benches/ benches/
COPY --from=web-builder /web/dist web/dist
COPY *.rs .
RUN touch src/main.rs
RUN --mount=type=cache,id=yantrikclaw-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=yantrikclaw-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=yantrikclaw-target,target=/app/target,sharing=locked \
    rm -rf target/release/.fingerprint/yantrikclawlabs-* \
           target/release/deps/yantrikclawlabs-* \
           target/release/incremental/yantrikclawlabs-* && \
    if [ -n "$YANTRIKCLAW_CARGO_FEATURES" ]; then \
      cargo build --release --locked --features "$YANTRIKCLAW_CARGO_FEATURES"; \
    else \
      cargo build --release --locked; \
    fi && \
    cp target/release/yantrikclaw /app/yantrikclaw && \
    strip /app/yantrikclaw
RUN size=$(stat -c%s /app/yantrikclaw) && \
    if [ "$size" -lt 1000000 ]; then echo "ERROR: binary too small (${size} bytes), likely dummy build artifact" && exit 1; fi

# Prepare runtime directory structure and default config inline (no extra stage)
RUN mkdir -p /yantrikclaw-data/.yantrikclaw /yantrikclaw-data/workspace && \
    printf '%s\n' \
        'workspace_dir = "/yantrikclaw-data/workspace"' \
        'config_path = "/yantrikclaw-data/.yantrikclaw/config.toml"' \
        'api_key = ""' \
        'default_provider = "openrouter"' \
        'default_model = "anthropic/claude-sonnet-4-20250514"' \
        'default_temperature = 0.7' \
        '' \
        '[gateway]' \
        'port = 42617' \
        'host = "[::]"' \
        'allow_public_bind = true' \
        > /yantrikclaw-data/.yantrikclaw/config.toml && \
    chown -R 65534:65534 /yantrikclaw-data

# ── Stage 2: Development Runtime (Debian) ────────────────────
FROM debian:bookworm-slim AS dev

# Install essential runtime dependencies only (use docker-compose.override.yml for dev tools)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /yantrikclaw-data /yantrikclaw-data
COPY --from=builder /app/yantrikclaw /usr/local/bin/yantrikclaw

# Overwrite minimal config with DEV template (Ollama defaults)
COPY dev/config.template.toml /yantrikclaw-data/.yantrikclaw/config.toml
RUN chown 65534:65534 /yantrikclaw-data/.yantrikclaw/config.toml

# Environment setup
# Ensure UTF-8 locale so CJK / multibyte input is handled correctly
ENV LANG=C.UTF-8
# Use consistent workspace path
ENV YANTRIKCLAW_WORKSPACE=/yantrikclaw-data/workspace
ENV HOME=/yantrikclaw-data
# Defaults for local dev (Ollama) - matches config.template.toml
ENV PROVIDER="ollama"
ENV YANTRIKCLAW_MODEL="llama3.2"
ENV YANTRIKCLAW_GATEWAY_PORT=42617

# Note: API_KEY is intentionally NOT set here to avoid confusion.
# It is set in config.toml as the Ollama URL.

WORKDIR /yantrikclaw-data
USER 65534:65534
EXPOSE 42617
HEALTHCHECK --interval=60s --timeout=10s --retries=3 --start-period=10s \
    CMD ["yantrikclaw", "status", "--format=exit-code"]
ENTRYPOINT ["yantrikclaw"]
CMD ["daemon"]

# ── Stage 3: Production Runtime (Distroless) ─────────────────
FROM gcr.io/distroless/cc-debian12:nonroot AS release

COPY --from=builder /app/yantrikclaw /usr/local/bin/yantrikclaw
COPY --from=builder /yantrikclaw-data /yantrikclaw-data

# Environment setup
# Ensure UTF-8 locale so CJK / multibyte input is handled correctly
ENV LANG=C.UTF-8
ENV YANTRIKCLAW_WORKSPACE=/yantrikclaw-data/workspace
ENV HOME=/yantrikclaw-data
# Default provider and model are set in config.toml, not here,
# so config file edits are not silently overridden
#ENV PROVIDER=
ENV YANTRIKCLAW_GATEWAY_PORT=42617

# API_KEY must be provided at runtime!

WORKDIR /yantrikclaw-data
USER 65534:65534
EXPOSE 42617
HEALTHCHECK --interval=60s --timeout=10s --retries=3 --start-period=10s \
    CMD ["yantrikclaw", "status", "--format=exit-code"]
ENTRYPOINT ["yantrikclaw"]
CMD ["daemon"]
