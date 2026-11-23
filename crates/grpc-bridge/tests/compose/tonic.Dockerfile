# The tonic client of docker-compose.yml (zerodds-grpc-bridge-1.0 §12.3):
# crates/grpc-bridge/tests/interop/tonic with its lock file, stubs from
# crates/grpc-bridge/proto by tonic-prost-build and protoc.
FROM rust:1.97.1-slim-trixie AS build
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY crates/grpc-bridge/proto crates/grpc-bridge/proto
COPY crates/grpc-bridge/tests/interop/tonic crates/grpc-bridge/tests/interop/tonic
RUN cargo build --release --locked --manifest-path crates/grpc-bridge/tests/interop/tonic/Cargo.toml

FROM debian:trixie-slim
COPY --from=build /src/crates/grpc-bridge/tests/interop/tonic/target/release/tonic-client /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/tonic-client"]
