# Build multi-stage avec cargo-chef : la couche de compilation des
# dépendances est mise en cache tant que Cargo.toml/Cargo.lock ne bougent pas.
# Contexte de build : apps/api

FROM lukemathwalker/cargo-chef:latest AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin lebontroc-api

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home appuser
COPY --from=builder /app/target/release/lebontroc-api /usr/local/bin/lebontroc-api

ARG BUILD_SHA=""
ENV BUILD_SHA=$BUILD_SHA

USER appuser
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --retries=3 --start-period=15s \
    CMD curl -fsS http://127.0.0.1:8080/health || exit 1
CMD ["lebontroc-api"]
