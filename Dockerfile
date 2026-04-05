# syntax=docker/dockerfile:1

# Security (operational): rebuild with `docker build --pull` for base image patches; scan images (Trivy,
# Grype, Docker Scout); never pass real secrets via ARG/ENV here—they persist in image history. Prefer
# orchestrator-native secrets over plain env in production; env vars remain visible via `docker inspect`.
# This runtime has no shell or curl: probe GET /healthz from a load balancer or monitor outside the container.

# 1.88+ for `if let`/`&&` in the codebase; lockfile/deps may need newer (e.g. 1.94)—bump if build fails.
ARG RUST_VERSION=1.94

FROM rust:${RUST_VERSION}-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    git \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
# Cargo requires at least one target to parse the manifest before fetch.
RUN mkdir -p src \
    && printf '%s\n' 'pub fn lib_placeholder() {}' > src/lib.rs \
    && printf '%s\n' 'fn main() {}' > src/main.rs \
    && cargo fetch --locked \
    && rm -rf src

COPY src ./src
ENV RUSTFLAGS="-C strip=symbols"
RUN cargo build --release --locked

FROM gcr.io/distroless/cc-debian12:nonroot

LABEL org.opencontainers.image.title="ratatoskr" \
    org.opencontainers.image.description="Webhook worker for secret delivery and lifecycle orchestration"

COPY --from=builder --chown=nonroot:nonroot /build/target/release/ratatoskr /usr/local/bin/ratatoskr

ENV RATATOSKR_CONFIG=/etc/ratatoskr/ratatoskr.toml

USER nonroot
EXPOSE 8080
CMD ["/usr/local/bin/ratatoskr"]
