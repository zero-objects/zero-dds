# packaging/docker/durability-svc/Dockerfile — zerodds/durability-svc:1.0
# Standalone ZeroDDS Durability-Service daemon (ADR 0009).
#
# Note: the lakehouse adapter bundles DuckDB, compiled from C++ source — the
# builder needs cmake + a C++ toolchain, and the runtime needs libstdc++6.
FROM rust:1.88-bookworm AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
        cmake build-essential \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-chef --version 0.1.77 --locked
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release -p zerodds-durability-service-bin --recipe-path recipe.json
COPY . .
RUN cargo build --release -p zerodds-durability-service-bin

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 libstdc++6 \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/lib/zerodds/durability
COPY --from=builder /build/target/release/zerodds-durability-svc /usr/local/bin/
# Default: domain 0, sqlite at the mounted volume, auto-discovery on.
# Override via `docker run … zerodds-durability-svc --domain N --store … --path …`.
VOLUME /var/lib/zerodds/durability
ENTRYPOINT ["zerodds-durability-svc"]
CMD ["--domain", "0", "--store", "sqlite", "--path", "/var/lib/zerodds/durability/dur.db", "--auto"]
