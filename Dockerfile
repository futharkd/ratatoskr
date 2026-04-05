# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY mimir mimir
COPY ratatoskr ratatoskr
RUN cargo build -p ratatoskr --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 1000 --home-dir /var/lib/ratatoskr --shell /usr/sbin/nologin ratatoskr \
    && mkdir -p /etc/ratatoskr /var/lib/ratatoskr \
    && chown -R ratatoskr:ratatoskr /var/lib/ratatoskr

COPY --from=builder /app/target/release/ratatoskr /usr/local/bin/ratatoskr

USER ratatoskr
ENV RATATOSKR_CONFIG=/etc/ratatoskr/ratatoskr.toml
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD wget -qO- http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/ratatoskr"]
