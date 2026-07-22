#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Runs the ZeroDDS native-endpoint examples end-to-end, BOTH directions (ADR
# 0013): C / Python / Java endpoints publish a SensorReading to a real
# zerodds-xrce hub over UDP, and the same endpoints receive a sample the hub
# pushes. Asserts all six exchanges.
#   endpoints/examples/run.sh [base-port]
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
PORT=${1:-17490}
HUB="$ROOT/target/debug/zerodds-xrce-agent-demo"
TO=$(command -v timeout || command -v gtimeout || true)
CINC="-I$ROOT/endpoints/c/include -I$ROOT/endpoints/c/test"
CSRC="$ROOT/endpoints/c/src/zerodds_wire.c $ROOT/endpoints/c/src/zerodds_endpoint.c $ROOT/endpoints/c/test/sample_sensor.c"

echo "== building hub + endpoints =="
cargo build -q -p zerodds-xrce-agent-demo
gcc -O2 $CINC $CSRC "$ROOT/endpoints/c/examples/udp_endpoint.c" -o /tmp/zdw_c_pub
gcc -O2 $CINC $CSRC "$ROOT/endpoints/c/examples/udp_receiver.c" -o /tmp/zdw_c_recv
javac -d /tmp/zdw_java "$ROOT/endpoints/java/Zdw.java" "$ROOT/endpoints/java/ZdwEndpoint.java" \
    "$ROOT/endpoints/examples/PublishUdp.java" "$ROOT/endpoints/examples/ReceiveUdp.java"

echo ""
echo "== PUBLISH (endpoint -> hub): C / Python / Java =="
$TO ${TO:+20} "$HUB" recv "$PORT" 3 > /tmp/zdw_hub.out 2>&1 &
HUB_PID=$!
sleep 1
/tmp/zdw_c_pub 127.0.0.1 "$PORT"
python3 "$ROOT/endpoints/examples/publish_udp.py" 127.0.0.1 "$PORT"
java -cp /tmp/zdw_java PublishUdp 127.0.0.1 "$PORT"
wait "$HUB_PID" 2>/dev/null || true
cat /tmp/zdw_hub.out
PUB_OK=$(grep -c "AGENT OK" /tmp/zdw_hub.out || true)

echo ""
echo "== RECEIVE (hub -> endpoint): C / Python / Java =="
RECV_OK=0
recv_one() { # $1 = label, $2 = command that binds $3, $3 = port
    local out="/tmp/zdw_recv_$1.out"
    $TO ${TO:+10} bash -c "$2" > "$out" 2>&1 &
    local pid=$!
    sleep 1
    "$HUB" send 127.0.0.1 "$3" >/dev/null
    wait "$pid" 2>/dev/null || true
    cat "$out"
    grep -q "RECEIVER OK" "$out" && RECV_OK=$((RECV_OK + 1))
}
recv_one c "/tmp/zdw_c_recv $((PORT+1))" $((PORT+1))
recv_one py "python3 '$ROOT/endpoints/examples/receive_udp.py' $((PORT+2))" $((PORT+2))
recv_one java "java -cp /tmp/zdw_java ReceiveUdp $((PORT+3))" $((PORT+3))

echo ""
if [ "$PUB_OK" -eq 3 ] && [ "$RECV_OK" -eq 3 ]; then
    echo "EXAMPLES OK: publish 3/3 + receive 3/3 (C/Python/Java <-> Rust hub)"
else
    echo "EXAMPLES FAILED: publish $PUB_OK/3, receive $RECV_OK/3"; exit 1
fi
