# The Cyclone DDS participant of docker-compose.yml (zerodds-grpc-bridge-1.0
# §12.3): cyclone_chat.c on Debian's Cyclone DDS 0.10, built with idlc
# (cyclonedds-tools).
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends cyclonedds-dev cyclonedds-tools gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY crates/grpc-bridge/tests/interop/chat.idl crates/grpc-bridge/tests/interop/cyclone_chat.c ./
RUN idlc chat.idl && cc -o /usr/local/bin/cyclone_chat cyclone_chat.c chat.c -I. -lddsc
ENTRYPOINT ["/usr/local/bin/cyclone_chat"]
