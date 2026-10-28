# packaging/docker/py-runtime/Dockerfile — sandbox runtime for the
# Python bindings (`zerodds-py` via PyO3 + maturin). Audience:
# learning and coding challenges in sandboxes, and quickstart demos.
#
# Unlike the daemon images (cli, ws-bridged, ...), this image ships
# no ENTRYPOINT service, but a ready Python environment
# with `import zerodds`. Learner code is mounted into /workspace or
# injected via docker exec / `command_file`.
#
# Build from the repository root:
#   docker build -f packaging/docker/py-runtime/Dockerfile -t zerodds/py-runtime:rc7 .

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
RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 python3-dev python3-venv python3-pip pkg-config \
    && rm -rf /var/lib/apt/lists/*
# Install maturin into its own venv — avoids a conflict with the
# Debian system python and needs no --break-system-packages workaround.
RUN python3 -m venv /opt/maturin-venv \
    && /opt/maturin-venv/bin/pip install --no-cache-dir maturin==1.8.*
ENV PATH=/opt/maturin-venv/bin:$PATH
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p zerodds-py
COPY . .
RUN python3 packaging/python/sync-version.py
# ABI3 has a 3.8 lower bound; the complete distribution needs Python 3.9+.
WORKDIR /build/crates/py
RUN maturin build --release --out /build/wheels
WORKDIR /build
RUN python3 -m pip wheel --no-deps --wheel-dir /build/wheels runtime/python

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 python3-venv ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

# Install the zerodds wheel into a dedicated venv — cleaner than
# --break-system-packages, and it provides a known `python3` via PATH.
COPY --from=builder /build/wheels/*.whl /tmp/
RUN python3 -m venv /opt/zerodds \
    && /opt/zerodds/bin/pip install --no-index --find-links=/tmp /tmp/*.whl \
    && rm -rf /tmp/*.whl

# Read-only root compatibility: no .pyc writes into site-packages,
# output passes straight through (no stdout buffer for sandbox logs).
ENV PATH=/opt/zerodds/bin:$PATH \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1

WORKDIR /workspace

# Tini as PID 1: reaps zombie children in learner multi-process demos
# (pub + sub as separate processes in the same container).
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["python3"]
