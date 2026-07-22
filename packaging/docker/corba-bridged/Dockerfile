# packaging/docker/corba-bridged/Dockerfile — Spec §5.2.
FROM rust:1.88-bookworm AS chef
# iceoryx2-pal-posix (pulled into the workspace recipe via the
# iceoryx2-oracle-consumer tool) runs bindgen in its build script, which needs
# libclang. The slim rust image doesn't ship it — GH-hosted runners do, which is
# why this only bites inside the container. cargo-chef cook builds the full
# recipe, so libclang must be present here even though no shipped bin uses it.
RUN apt-get update && apt-get install -y --no-install-recommends clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --version 0.1.77 --locked
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin zerodds-corba-bridged
COPY . .
RUN cargo build --release --bin zerodds-corba-bridged

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 tini \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 999 --home /var/lib/zerodds --shell /usr/sbin/nologin zerodds
COPY --from=builder /build/target/release/zerodds-corba-bridged /usr/local/bin/
COPY packaging/linux/configs/corba-bridged.yaml.example /etc/zerodds/corba-bridged.yaml
RUN mkdir -p /var/log/zerodds /var/lib/zerodds /etc/zerodds/certs \
    && chown -R zerodds:zerodds /var/log/zerodds /var/lib/zerodds /etc/zerodds
USER zerodds
EXPOSE 2809 9096
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/zerodds-corba-bridged"]
CMD ["--config", "/etc/zerodds/corba-bridged.yaml"]
