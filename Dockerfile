# syntax=docker/dockerfile:1.7

# doltclaw — minimal agent runtime: LLM inference, model fallback, tool calling.
# Multi-stage: rust:1.99 builder → debian slim runtime. Non-root runtime user.
# Builds the workspace release binary; runs `doltclaw` CLI.

# ---------- builder ----------
FROM rust:1.99-bookworm AS builder

WORKDIR /app

# Prefetch deps against the lockfile before copying source, so this layer caches.
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# --locked: honor the committed lockfile; fail rather than re-resolve.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --features cli && \
    cp /app/target/release/doltclaw /tmp/doltclaw

# ---------- runtime ----------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --no-create-home --uid 10001 doltclaw

WORKDIR /srv/doltclaw
COPY --from=builder /tmp/doltclaw /usr/local/bin/doltclaw

USER 10001

ENV RUST_LOG=info

ENTRYPOINT ["doltclaw"]
