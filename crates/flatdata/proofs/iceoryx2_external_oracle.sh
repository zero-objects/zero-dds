#!/usr/bin/env bash
# External iceoryx2 oracle proof.
#
# Builds two SEPARATE binaries with disjoint dependency graphs:
#   * iceoryx2-oracle-consumer  — depends ONLY on the upstream iceoryx2 crate
#                                 (tools/iceoryx2-oracle-consumer/Cargo.toml),
#                                 zero ZeroDDS code. The independent consumer.
#   * iceoryx2_oracle_publisher — a zerodds-flatdata example that publishes
#                                 via RawIceoryx2Publisher.
#
# The consumer subscribes by verbatim service name, the ZeroDDS publisher sends,
# and the consumer asserts it read the exact bytes. A pass proves ZeroDDS emits
# genuine iceoryx2 wire (service variant + name + chunk payload) that an
# external iceoryx2 application consumes — not just ZeroDDS-to-ZeroDDS.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../../.." && pwd)"
SERVICE="zerodds_iox_oracle_$$"
PAYLOAD="010203040506deadbeef"
READY="$(mktemp -t iox_oracle_ready.XXXXXX)"
rm -f "$READY"

echo "==> building external consumer (iceoryx2-only) + ZeroDDS publisher"
cargo build -q -p iceoryx2-oracle-consumer
cargo build -q -p zerodds-flatdata --features iceoryx2-bridge --example iceoryx2_oracle_publisher

CONSUMER="$REPO/target/debug/iceoryx2-oracle-consumer"
PUBLISHER="$REPO/target/debug/examples/iceoryx2_oracle_publisher"

echo "==> service=$SERVICE payload=$PAYLOAD"

# Start the external consumer first; it signals readiness via $READY.
ORACLE_READY_FILE="$READY" "$CONSUMER" "$SERVICE" "$PAYLOAD" 15000 &
CONSUMER_PID=$!

# Wait (bounded) for the subscriber to be live before publishing — iceoryx2
# keeps no late-joiner history by default.
for _ in $(seq 1 100); do
  [ -f "$READY" ] && break
  sleep 0.1
done

echo "==> publishing from ZeroDDS"
"$PUBLISHER" "$SERVICE" "$PAYLOAD" 50 50 || true

if wait "$CONSUMER_PID"; then
  echo "PROOF: external iceoryx2 consumer read the ZeroDDS sample — PASS"
  rm -f "$READY"
  exit 0
else
  echo "PROOF: external iceoryx2 consumer did NOT read the ZeroDDS sample — FAIL"
  rm -f "$READY"
  exit 1
fi
