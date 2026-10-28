# Isolation smoke test Docker image (WP 2.0b T4).
#
# Minimal Debian image with the `isolation-smoke` binary. The image
# is consumed by docker-compose.*.yml, once per transport.
FROM debian:12-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain 1.85.0 --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /src
COPY . .
RUN cargo build --release -p dds-isolation-smoke

FROM debian:12-slim
COPY --from=builder /src/target/release/isolation-smoke /usr/local/bin/isolation-smoke
ENTRYPOINT ["/usr/local/bin/isolation-smoke"]
