# packaging/docker/ts-node-runtime/Dockerfile — sandbox runtime
# for the TypeScript bindings under Node.js. `@zerodds/node` (from
# crates/ts-node) is installed globally, libzerodds.so is found through
# the koffi FFI, zerodds-idlc is there for `--lang typescript`, and tsx
# runs TypeScript inline without a separate build step.
#
# Build from the repository root:
#   docker build -f packaging/docker/ts-node-runtime/Dockerfile -t zerodds/ts-node-runtime:rc7 .

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

FROM chef AS rust-builder
COPY --from=planner /build/recipe.json recipe.json
# Two packages: zerodds-c-api (cdylib for the koffi FFI) and zerodds-idlc
# (bin for learner code generation). cargo allows -p more than once; --bin
# works only if the bin lies in the -p package — across packages
# the build would fail otherwise.
RUN cargo chef cook --release --recipe-path recipe.json \
    -p zerodds-c-api -p zerodds-idlc
COPY . .
RUN cargo build --release -p zerodds-c-api -p zerodds-idlc

FROM node:22-bookworm-slim AS node-builder
WORKDIR /build
COPY crates/ts-node/package.json crates/ts-node/package-lock.json* \
     crates/ts-node/tsconfig.json /build/
RUN npm install --no-audit --no-fund
COPY crates/ts-node/src /build/src
COPY crates/ts-node/test /build/test
RUN npm run build
# `npm pack` packs the distribution bundle (dist/ + src/ + package.json
# per the "files" list) into a versioned .tgz, which we install
# as a global package in the runtime stage.
RUN npm pack && mv zerodds-node-*.tgz /build/zerodds-node.tgz

FROM node:22-bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

# Native lib from the Rust builder. koffi loads it at runtime through
# LD_LIBRARY_PATH; ldconfig makes sure that dlopen() without an
# explicit path works too.
COPY --from=rust-builder /build/target/release/libzerodds.so /usr/local/lib/libzerodds.so
RUN echo "/usr/local/lib" > /etc/ld.so.conf.d/zerodds.conf && ldconfig

# IDL compiler for learners (`zerodds-idlc generate --lang typescript --out out chat.idl`).
COPY --from=rust-builder /build/target/release/zerodds-idlc /usr/local/bin/zerodds-idlc

# Install @zerodds/node globally — a learner can work via
# `import { ... } from '@zerodds/node'` without a local node_modules.
# tsx runs TS directly without a tsc build.
COPY --from=node-builder /build/zerodds-node.tgz /tmp/zerodds-node.tgz
RUN npm install -g --omit=dev /tmp/zerodds-node.tgz tsx@4 \
    && rm /tmp/zerodds-node.tgz

ENV LD_LIBRARY_PATH=/usr/local/lib \
    NODE_PATH=/usr/local/lib/node_modules

WORKDIR /workspace

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/bin/bash"]
