# packaging/docker/grpc-bridged/Dockerfile — Spec §5.2.
FROM rust:1.88-bookworm AS chef
RUN cargo install cargo-chef --version 0.1.66 --locked
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin zerodds-grpc-bridged
COPY . .
RUN cargo build --release --bin zerodds-grpc-bridged

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 tini \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 999 --home /var/lib/zerodds --shell /usr/sbin/nologin zerodds
COPY --from=builder /build/target/release/zerodds-grpc-bridged /usr/local/bin/
COPY packaging/linux/configs/grpc-bridged.yaml.example /etc/zerodds/grpc-bridged.yaml
RUN mkdir -p /var/log/zerodds /var/lib/zerodds /etc/zerodds/certs \
    && chown -R zerodds:zerodds /var/log/zerodds /var/lib/zerodds /etc/zerodds
USER zerodds
EXPOSE 50051 9095
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/zerodds-grpc-bridged"]
CMD ["--config", "/etc/zerodds/grpc-bridged.yaml"]
