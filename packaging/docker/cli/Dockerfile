# packaging/docker/cli/Dockerfile — Spec §5.1 (zerodds/cli:1.0).
# Bundles all currently-available CLI tools — useful for kubectl exec /
# debug pods. Bin-list is restricted to packages actually present in
# the workspace (verified via `cargo metadata --no-deps`).
FROM rust:1.88-bookworm AS chef
RUN cargo install cargo-chef --version 0.1.77 --locked
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release \
    --bin zerodds-admin --bin zerodds-idlc --bin zerodds-replay \
    --bin zerodds-perf --bin zerodds-xmlc --bin zerodds-traceability \
    --bin zerodds-lint --bin zerodds-chaos --bin zerodds-dashboard \
    --bin zerodds-recorder-bridge --bin zerodds-ros2-shim

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/zerodds-admin            /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-idlc             /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-replay           /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-perf             /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-xmlc             /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-traceability     /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-lint             /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-chaos            /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-dashboard        /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-recorder-bridge  /usr/local/bin/
COPY --from=builder /build/target/release/zerodds-ros2-shim        /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/zerodds-admin"]
CMD ["--help"]
