# packaging/docker/ts-node-runtime/Dockerfile — RC3 Sandbox-Runtime
# fuer TypeScript-Bindings unter Node.js. `@zerodds/node` (aus
# crates/ts-node) liegt global installiert, libzerodds.so wird ueber
# koffi-FFI gefunden, zerodds-idlc fuer `--ts`-Codegen ist da, tsx
# fuer Inline-TypeScript-Run ohne separaten Build-Step.
#
# Build vom Repo-Root:
#   docker build -f packaging/docker/ts-node-runtime/Dockerfile -t zerodds/ts-node-runtime:rc3 .

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
# Zwei Packages: zerodds-c-api (cdylib fuer koffi-FFI) und zerodds-idlc
# (bin fuer Lerner-Codegen). cargo erlaubt mehrfaches -p; --bin geht
# nur, wenn die bin im -p-Package liegt — fuer Cross-Package wuerde
# der Build sonst scheitern.
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
# `npm pack` packt das Distribution-Bundle (dist/ + src/ + package.json
# laut "files"-Liste) in ein versioniertes .tgz, das wir in der
# Runtime-Stage als globales Package installieren.
RUN npm pack && mv zerodds-node-*.tgz /build/zerodds-node.tgz

FROM node:22-bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

# Native Lib aus dem Rust-Builder. koffi laedt sie zur Laufzeit ueber
# LD_LIBRARY_PATH; ldconfig stellt sicher, dass auch dlopen() ohne
# expliziten Pfad geht.
COPY --from=rust-builder /build/target/release/libzerodds.so /usr/local/lib/libzerodds.so
RUN echo "/usr/local/lib" > /etc/ld.so.conf.d/zerodds.conf && ldconfig

# IDL-Compiler fuer Lerner (`zerodds-idlc --ts -o out chat.idl`).
COPY --from=rust-builder /build/target/release/zerodds-idlc /usr/local/bin/zerodds-idlc

# @zerodds/node global installieren — Lerner kann via
# `import { ... } from '@zerodds/node'` ohne lokales node_modules
# arbeiten. tsx liefert TS-Direct-Run ohne tsc-Build.
COPY --from=node-builder /build/zerodds-node.tgz /tmp/zerodds-node.tgz
RUN npm install -g --omit=dev /tmp/zerodds-node.tgz tsx@4 \
    && rm /tmp/zerodds-node.tgz

ENV LD_LIBRARY_PATH=/usr/local/lib \
    NODE_PATH=/usr/local/lib/node_modules

WORKDIR /workspace

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/bin/bash"]
