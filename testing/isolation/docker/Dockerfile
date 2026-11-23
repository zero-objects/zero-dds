# Isolation smoke test Docker image.
#
# Debian trixie with the `isolation-smoke` binary of
# tools/isolation-smoke, built with the toolchain of rust-toolchain.toml.
# The image is consumed by docker-compose.*.yml, once per transport.
FROM rust:1.97.1-slim-trixie AS builder

WORKDIR /src
COPY . .
RUN cargo build --release --locked -p zerodds-isolation-smoke

FROM debian:trixie-slim
COPY --from=builder /src/target/release/isolation-smoke /usr/local/bin/isolation-smoke
ENTRYPOINT ["/usr/local/bin/isolation-smoke"]
