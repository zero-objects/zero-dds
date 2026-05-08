# packaging/docker/cli/Dockerfile — Spec §5.1 (zerodds/cli:1.0).
# Bundles all 17 CLI tools — useful for kubectl exec / debug pods.
FROM rust:1.88-bookworm AS chef
RUN cargo install cargo-chef --version 0.1.66 --locked
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release \
    --bin zerodds-admin --bin zerodds-bench --bin zerodds-bench-suite \
    --bin zerodds-idlc --bin zerodds-spy --bin zerodds-record \
    --bin zerodds-replay --bin zerodds-snitch --bin zerodds-monitor \
    --bin zerodds-mq --bin zerodds-pcap --bin zerodds-perf \
    --bin zerodds-secure-keygen --bin zerodds-secure-permissions \
    --bin zerodds-typeobject --bin zerodds-xml-config \
    --bin zerodds-ros2-shim

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/zerodds-admin              /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-bench              /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-bench-suite        /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-idlc               /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-spy                /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-record             /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-replay             /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-snitch             /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-monitor            /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-mq                 /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-pcap               /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-perf               /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-secure-keygen      /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-secure-permissions /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-typeobject         /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-xml-config         /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-ros2-shim          /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/zerodds-admin"]
CMD ["--help"]
