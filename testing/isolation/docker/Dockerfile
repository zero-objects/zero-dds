# Isolation-Smoke-Test Docker Image (WP 2.0b T4).
#
# Minimales Debian-Image mit dem `isolation-smoke`-Binary. Das Image
# wird von docker-compose.*.yml pro Transport konsumiert.
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
