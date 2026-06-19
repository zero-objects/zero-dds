#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Java↔Rust multi-process e2e for the gRPC DDS bridge (F2b).
#
#   1. builds `zerodds-grpc-bridged --features dds-runtime`
#   2. starts it on a free port with a DemoStream topic
#   3. runs the Maven `GrpcBridgeClientTest` (pure-Java gRPC client) against it
#   4. tears the daemon down
#
# The Java test publishes via gRPC → DDS DataWriter → loopback DataReader →
# gRPC Subscribe and asserts the round-tripped payload. Run from anywhere.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
JAVA_DIR="$REPO_ROOT/crates/java-omgdds/java"

echo "==> building daemon (dds-runtime)"
cargo build --manifest-path "$REPO_ROOT/Cargo.toml" \
    -p zerodds-grpc-bridge --bin zerodds-grpc-bridged --features dds-runtime

DAEMON="$REPO_ROOT/target/debug/zerodds-grpc-bridged"
[ -x "$DAEMON" ] || { echo "daemon binary not found: $DAEMON" >&2; exit 1; }

PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
DOMAIN=$(( PORT % 200 ))
[ "$DOMAIN" -lt 1 ] && DOMAIN=1

echo "==> starting daemon on 127.0.0.1:$PORT (domain $DOMAIN)"
"$DAEMON" --bind "127.0.0.1:$PORT" --domain "$DOMAIN" \
    --topic "Demo::Bytes=DemoStream" --idle-timeout-ms 60000 \
    >/tmp/zerodds-bridge-e2e.log 2>&1 &
DAEMON_PID=$!
trap 'kill "$DAEMON_PID" 2>/dev/null || true' EXIT

# Wait for the participant + listener to come up.
sleep 1.5

echo "==> running Maven GrpcBridgeClientTest"
cd "$JAVA_DIR"
ZERODDS_BRIDGE_PORT="$PORT" mvn -q -Dtest=GrpcBridgeClientTest test
RC=$?

echo "==> e2e exit code: $RC"
exit $RC
