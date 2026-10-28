# packaging/docker/cpp-runtime/Dockerfile — sandbox runtime for the
# C/C++ bindings. Ships libzerodds.so + libzerodds_wire.a (the one
# C++ wire codec, runtime/cpp) + C headers (zerodds.h, zerodds_xcdr2.h)
# + C++ RAII headers (zerodds/dds.hpp + dds/dds.hpp) plus zerodds-idlc
# for IDL→C++ code generation, and a build toolchain (clang + cmake),
# so that learners can compile their own C/C++ code in the
# container.
#
# Unlike the daemon images: no ENTRYPOINT service, but a
# ready toolchain. Learner code is mounted into /workspace or
# injected via docker exec / command_file.
#
# Build from the repository root:
#   docker build -f packaging/docker/cpp-runtime/Dockerfile -t zerodds/cpp-runtime:rc7 .

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
# Two packages: zerodds-c-api (cdylib) and zerodds-idlc (bin). cargo
# allows -p more than once; --bin works only if the bin lies in the -p
# package — across packages the build would fail otherwise.
RUN cargo chef cook --release --recipe-path recipe.json \
    -p zerodds-c-api -p zerodds-idlc
COPY . .
RUN cargo build --release -p zerodds-c-api -p zerodds-idlc
# The one C++ wire codec (runtime/cpp) a learner's `zerodds::TypedWriter<T>`/
# `TypedReader<T>` sample (via zerodds-idlc generate --lang c++ --cpp-mapping native) links
# against — zerodds/dds.hpp declares it, libzerodds.so (the Rust cdylib)
# cannot contain it, so it ships as its own static archive.
RUN mkdir -p /build/target/cpp-wire && \
    for f in writer reader float128 md5; do \
        clang++ -std=c++17 -O2 -Iruntime/cpp/include \
            -c runtime/cpp/src/$f.cpp -o /build/target/cpp-wire/$f.o; \
    done && \
    ar rcs /build/target/cpp-wire/libzerodds_wire.a /build/target/cpp-wire/*.o

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tini \
        clang lld cmake make pkg-config \
        libc6-dev libstdc++-12-dev \
    && rm -rf /var/lib/apt/lists/*

# Native lib + headers. The CMake paths follow FHS:
#   /usr/local/lib/libzerodds.so          — DSO
#   /usr/local/include/zerodds.h          — C FFI header
#   /usr/local/include/zerodds_xcdr2.h    — typed dispatch contract (C)
#   /usr/local/include/zerodds/           — C++17 RAII headers + the one
#                                            C++ wire codec (wire.hpp,
#                                            runtime/cpp)
#   /usr/local/include/dds/               — OMG DDS-PSM-Cxx headers
COPY --from=builder /build/target/release/libzerodds.so /usr/local/lib/libzerodds.so
COPY --from=builder /build/target/cpp-wire/libzerodds_wire.a /usr/local/lib/libzerodds_wire.a
COPY --from=builder /build/crates/zerodds-c-api/include/ /usr/local/include/
COPY --from=builder /build/crates/cpp/include/zerodds/   /usr/local/include/zerodds/
COPY --from=builder /build/crates/cpp/include/dds/       /usr/local/include/dds/
COPY --from=builder /build/runtime/cpp/include/zerodds/  /usr/local/include/zerodds/

# IDL compiler for learners (`zerodds-idlc generate --lang c++ --out out chat.idl`).
COPY --from=builder /build/target/release/zerodds-idlc /usr/local/bin/zerodds-idlc

# The linker does not find /usr/local/lib by default on Debian bookworm.
RUN echo "/usr/local/lib" > /etc/ld.so.conf.d/zerodds.conf && ldconfig

# C++ compiler wrapper convention: clang++ as the default, since it is
# compatible with C++17/20 and faster than g++ for sandbox builds.
ENV CC=clang \
    CXX=clang++ \
    LD_LIBRARY_PATH=/usr/local/lib

WORKDIR /workspace

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/bin/bash"]
