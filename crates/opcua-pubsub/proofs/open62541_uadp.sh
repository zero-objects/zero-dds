#!/usr/bin/env bash
# The ZeroDDS Part-14 UADP stack against open62541, both directions.
#
#   A: open62541's `tutorial_pubsub_publish` -> the ZeroDDS decoder.
#      Asserted against the foreign example's own configuration: PublisherId
#      UInt16 2234, WriterGroupId 100, DataSetWriterId 62541, a Variant DateTime
#      field (tutorial_pubsub_subscribe.c:81-85 spells out the same triple).
#
#   B: the ZeroDDS publisher -> open62541's `tutorial_pubsub_subscribe`, read
#      back by `uadp_target_check`, a client built against open62541 alone. No
#      ZeroDDS code is in B's assertion path.
#
# Requires an open62541 build with UA_BUILD_EXAMPLES=ON and UA_ENABLE_PUBSUB=ON.
# Direction B additionally needs UA_ENABLE_PUBSUB_INFORMATIONMODEL=OFF: the
# example hardcodes its target variables at ns=1;i=50000.., which the PubSub
# information model already occupies, so with it on the example's own
# addVariableNode calls fail and every received field is dropped with
# Bad_NodeIdUnknown. That is a property of the foreign example, not of the wire.
#
#   cmake -S <src> -B <build> -DUA_BUILD_EXAMPLES=ON -DUA_ENABLE_PUBSUB=ON \
#         -DUA_ENABLE_PUBSUB_INFORMATIONMODEL=OFF -DCMAKE_BUILD_TYPE=Release
#   make -C <build> -j
#
# `build-open62541.sh` beside this file does exactly that, and does it
# twice: the OPC-UA server proof wants the information model **on**, this
# one wants it off, and the same build directory cannot serve both. The
# defaults below therefore name the `-nopim` build. Pointing them at the
# other one is the failure the paragraph above describes, and it prints
# no error — every field arrives and is dropped.
#
# O62_EXAMPLES  <build>/bin/examples
# O62_LIB       <build>/bin              (libopen62541.a, for direction B)
# O62_SRC       <open62541 source tree>  (headers, for direction B)
# O62_GEN       <build>/src_generated    (generated headers, for direction B)
# O62_IFACE     local address to join the group on (224.0.0.x is link-local)
set -uo pipefail

EX="${O62_EXAMPLES:-/root/o62-ex2/bin/examples}"
LIB="${O62_LIB:-/root/o62-ex2/bin}"
SRC="${O62_SRC:-/root/open62541}"
GEN="${O62_GEN:-/root/o62-ex2/src_generated}"
GROUP="${O62_GROUP:-224.0.0.22:4840}"
IFACE="${O62_IFACE:-0.0.0.0}"
INT32=1234567
INT64=-9876543210

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
LOGDIR="$(mktemp -d -t opcua_uadp.XXXXXX)"

if [ ! -x "$EX/tutorial_pubsub_publish" ] || [ ! -x "$EX/tutorial_pubsub_subscribe" ]; then
  echo "PROOF-SKIP: open62541 PubSub examples not found under $EX"
  echo "            (set O62_EXAMPLES; this is a skip, not a pass)"
  exit 77
fi

PEER=
cleanup() { [ -n "$PEER" ] && kill -INT "$PEER" 2>/dev/null; }
trap cleanup EXIT

# --- Step 0: is the foreign publisher putting anything on the wire at all? ---
echo "==> step 0: the foreign publisher, observed without ZeroDDS code"
stdbuf -oL -eL "$EX/tutorial_pubsub_publish" > "$LOGDIR/pub0.log" 2>&1 &
PEER=$!
sleep 2
timeout 10 tcpdump -i any -n -c 3 "udp port ${GROUP##*:}" > "$LOGDIR/tcpdump.log" 2>&1
SEEN=$?
kill -INT "$PEER" 2>/dev/null; wait "$PEER" 2>/dev/null; PEER=
if [ "$SEEN" -ne 0 ] || ! grep -q "UDP, length" "$LOGDIR/tcpdump.log"; then
  echo "PROOF-FAIL: no UADP datagram from the foreign publisher — dead counterpart"
  cat "$LOGDIR/tcpdump.log"
  exit 1
fi
echo "    the counterpart is on the wire"

# --- Direction A: foreign publisher -> ZeroDDS decoder. ---------------------
echo "==> A: open62541 publisher -> ZeroDDS decoder"
cargo build -q -p zerodds-opcua-pubsub --example open62541_uadp_probe || exit 1
stdbuf -oL -eL "$EX/tutorial_pubsub_publish" > "$LOGDIR/pubA.log" 2>&1 &
PEER=$!
sleep 2
"$REPO/target/debug/examples/open62541_uadp_probe" "$GROUP" 3 "$IFACE"
A=$?
kill -INT "$PEER" 2>/dev/null; wait "$PEER" 2>/dev/null; PEER=
[ "$A" -ne 0 ] && { echo "PROOF-FAIL: direction A (exit $A)"; exit 1; }

# --- Direction B: ZeroDDS publisher -> foreign subscriber, foreign readback. -
echo "==> B: ZeroDDS publisher -> open62541 subscriber"
CHECK="$LOGDIR/uadp_target_check"
if ! gcc -o "$CHECK" "$HERE/uadp_target_check.c" \
       -I"$GEN" -I"$SRC/include" -I"$SRC/plugins/include" \
       -L"$LIB" -lopen62541 > "$LOGDIR/cc.log" 2>&1; then
  echo "PROOF-SKIP: could not build the open62541-only verifier"
  cat "$LOGDIR/cc.log"
  exit 77
fi
cargo build -q -p zerodds-opcua-pubsub --example open62541_uadp_publisher || exit 1
stdbuf -oL -eL "$EX/tutorial_pubsub_subscribe" > "$LOGDIR/subB.log" 2>&1 &
PEER=$!
sleep 4
"$REPO/target/debug/examples/open62541_uadp_publisher" "$GROUP" 25 "$INT32" "$INT64" || exit 1
sleep 1
"$CHECK" "opc.tcp://127.0.0.1:4840" "$INT32" "$INT64" 2>&1 | grep -v "info/"
B=${PIPESTATUS[0]}
kill -INT "$PEER" 2>/dev/null; wait "$PEER" 2>/dev/null; PEER=
if [ "$B" -ne 0 ]; then
  echo "PROOF-FAIL: direction B (exit $B)"
  grep -i "error writing\|KeyFrame" "$LOGDIR/subB.log" | head -5
  exit 1
fi

echo "PROOF-PASS: both directions of the UADP wire against open62541"
exit 0
