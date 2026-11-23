# packaging/docker/grpc-bridged/Dockerfile — Spec §5.2.
FROM rust:1.97.1-bookworm AS chef
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
RUN cargo chef cook --release --recipe-path recipe.json --bin zerodds-grpc-bridged
COPY . .
RUN cargo build --release --bin zerodds-grpc-bridged

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 tini \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 999 --home /var/lib/zerodds --shell /usr/sbin/nologin zerodds
COPY --from=builder /build/target/release/zerodds-grpc-bridged /usr/local/bin/
# The example configuration serves cleartext h2c on 127.0.0.1:50051 inside
# the container: the daemon refuses h2c on any other address
# (zerodds-grpc-bridge-1.0 §4.1, §11), so `-p 50051:50051` does not reach it.
# Run the image with `--network host`, or with TLS on a non-loopback bind:
# mount a configuration over /etc/zerodds/grpc-bridged.yaml, or mount the
# certificate and key under /etc/zerodds/certs and give the command (it
# replaces CMD)
#   --config /etc/zerodds/grpc-bridged.yaml
#   --set grpc.bind=0.0.0.0:50051 --set grpc.tls.enabled=true
# and an auth mode other than none, which a non-loopback bind refuses
# (§7.2), for example
#   --set auth.mode=mtls --set grpc.tls.client_ca_file=/etc/zerodds/certs/client-ca.pem
COPY packaging/linux/configs/grpc-bridged.yaml.example /etc/zerodds/grpc-bridged.yaml
RUN mkdir -p /var/log/zerodds /var/lib/zerodds /etc/zerodds/certs \
    && chown -R zerodds:zerodds /var/log/zerodds /var/lib/zerodds /etc/zerodds
USER zerodds
EXPOSE 50051 9095
# SIGTERM starts the drain of grpc.drain_secs (default 30 s, §9.2); give
# `docker stop -t 40` (compose: stop_grace_period: 40s), the default 10 s
# ends it with SIGKILL.
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/zerodds-grpc-bridged"]
CMD ["--config", "/etc/zerodds/grpc-bridged.yaml"]
