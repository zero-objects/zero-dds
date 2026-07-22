# packaging/docker/cpp-runtime/Dockerfile — RC3 Sandbox-Runtime fuer
# C/C++-Bindings. Liefert libzerodds.so + C-Header (zerodds.h,
# zerodds_xcdr2.h) + C++-RAII-Header (zerodds/dds.hpp + dds/dds.hpp)
# plus zerodds-idlc fuer IDL→C++-Codegen, dazu eine Build-Toolchain
# (clang + cmake), damit Lerner ihren eigenen C/C++-Code im Container
# uebersetzen koennen.
#
# Anders als die Daemon-Images: kein ENTRYPOINT-Service, sondern eine
# bereite Toolchain. Lerner-Code wird in /workspace gemounted oder
# per docker exec / command_file injiziert.
#
# Build vom Repo-Root:
#   docker build -f packaging/docker/cpp-runtime/Dockerfile -t zerodds/cpp-runtime:rc3 .

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
# Zwei Packages: zerodds-c-api (cdylib) und zerodds-idlc (bin). cargo
# erlaubt mehrfaches -p; --bin geht nur, wenn die bin im -p-Package
# liegt — fuer Cross-Package wuerde der Build sonst scheitern.
RUN cargo chef cook --release --recipe-path recipe.json \
    -p zerodds-c-api -p zerodds-idlc
COPY . .
RUN cargo build --release -p zerodds-c-api -p zerodds-idlc

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tini \
        clang lld cmake make pkg-config \
        libc6-dev libstdc++-12-dev \
    && rm -rf /var/lib/apt/lists/*

# Native Lib + Header. CMake-Pfade folgen FHS:
#   /usr/local/lib/libzerodds.so          — DSO
#   /usr/local/include/zerodds.h          — C-FFI-Header
#   /usr/local/include/zerodds_xcdr2.h    — XCDR2-C-Header
#   /usr/local/include/zerodds/           — C++17 RAII-Header
#   /usr/local/include/dds/               — OMG DDS-PSM-Cxx-Header
COPY --from=builder /build/target/release/libzerodds.so /usr/local/lib/libzerodds.so
COPY --from=builder /build/crates/zerodds-c-api/include/ /usr/local/include/
COPY --from=builder /build/crates/cpp/include/zerodds/   /usr/local/include/zerodds/
COPY --from=builder /build/crates/cpp/include/dds/       /usr/local/include/dds/

# IDL-Compiler fuer Lerner (`zerodds-idlc --cpp -o out chat.idl`).
COPY --from=builder /build/target/release/zerodds-idlc /usr/local/bin/zerodds-idlc

# Linker findet /usr/local/lib nicht per default auf Debian-bookworm.
RUN echo "/usr/local/lib" > /etc/ld.so.conf.d/zerodds.conf && ldconfig

# C++-Compiler-Wrapper-Convention: clang++ als Default, da kompatibel
# mit C++17/20 und schneller als g++ fuer Sandbox-Builds.
ENV CC=clang \
    CXX=clang++ \
    LD_LIBRARY_PATH=/usr/local/lib

WORKDIR /workspace

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/bin/bash"]
