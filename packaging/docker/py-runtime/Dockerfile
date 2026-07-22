# packaging/docker/py-runtime/Dockerfile — RC3 Sandbox-Runtime fuer
# Python-Bindings (`zerodds-py` via PyO3 + maturin). Zielgruppe:
# Lern-/Coding-Challenges in Zero-Learn-Sandboxes und Quickstart-Demos.
#
# Anders als die Daemon-Images (cli, ws-bridged, ...) liefert dieses
# Image keinen ENTRYPOINT-Service, sondern eine bereite Python-Umgebung
# mit `import zerodds`. Lerner-Code wird in /workspace gemounted oder
# per docker exec / `command_file` injiziert.
#
# Build vom Repo-Root:
#   docker build -f packaging/docker/py-runtime/Dockerfile -t zerodds/py-runtime:rc3 .

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
# Maturin in eigenes venv installieren — vermeidet Konflikt mit dem
# Debian-system-python und hat keinen --break-system-packages-Workaround.
RUN python3 -m venv /opt/maturin-venv \
    && /opt/maturin-venv/bin/pip install --no-cache-dir maturin==1.8.*
ENV PATH=/opt/maturin-venv/bin:$PATH
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p zerodds-py
COPY . .
# Maturin baut das Wheel mit eingebettetem PyO3-Native-Modul (abi3 → ein
# Wheel deckt Python 3.8+ ab).
WORKDIR /build/crates/py
RUN maturin build --release --out /build/wheels
WORKDIR /build

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 python3-venv ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

# zerodds-Wheel in ein dediziertes venv installieren — sauberer als
# --break-system-packages und liefert ein bekanntes `python3` per PATH.
COPY --from=builder /build/wheels/*.whl /tmp/
RUN python3 -m venv /opt/zerodds \
    && /opt/zerodds/bin/pip install --no-cache-dir /tmp/*.whl \
    && rm -rf /tmp/*.whl

# Read-only-root-Vertraeglichkeit: kein .pyc-Schreiben in Site-Packages,
# Output direkt durch (kein stdout-Buffer fuer Sandbox-Logs).
ENV PATH=/opt/zerodds/bin:$PATH \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1

WORKDIR /workspace

# Tini als PID-1: reapt Zombie-Kinder bei Lerner-Multiproc-Demos
# (Pub+Sub als getrennte Prozesse im selben Container).
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["python3"]
