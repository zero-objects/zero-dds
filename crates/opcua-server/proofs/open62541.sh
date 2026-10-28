#!/usr/bin/env bash
# The ZeroDDS OPC-UA client against open62541's own server.
#
# Nothing in the assertion path is ours: the endpoint list, the variable value
# and the ObjectsFolder references all come from open62541's
# `tutorial_server_variable`. Step 0 runs the foreign server against the
# foreign client first — if that fails, the counterpart is dead and no result
# from step 1 means anything.
#
# Requires an open62541 build with UA_BUILD_EXAMPLES=ON:
#   cmake -S <open62541-src> -B <build> -DUA_BUILD_EXAMPLES=ON -DCMAKE_BUILD_TYPE=Release
#   make -C <build> -j
# Point O62_EXAMPLES at <build>/bin/examples.
#
# `crates/opcua-pubsub/proofs/build-open62541.sh` produces that build and
# one more. This proof wants the one **with** the PubSub information
# model, which is the plain `<src>-build`; the UADP proof wants the other
# one. Both are built from one source tree because they differ in a
# single cmake option.
set -uo pipefail

EX="${O62_EXAMPLES:-/root/o62-ex/bin/examples}"
PORT="${O62_PORT:-4840}"
REPO="$(cd "$(dirname "$0")/../../.." && pwd)"
LOGDIR="$(mktemp -d -t opcua_o62.XXXXXX)"

if [ ! -x "$EX/tutorial_server_variable" ] || [ ! -x "$EX/tutorial_client_firststeps" ]; then
  echo "PROOF-SKIP: open62541 examples not found under $EX"
  echo "            (set O62_EXAMPLES; this is a skip, not a pass)"
  exit 77
fi

cleanup() { [ -n "${SRV:-}" ] && kill -INT "$SRV" 2>/dev/null; }
trap cleanup EXIT

# --- Step 0: foreign against foreign, not one line of ZeroDDS code. ---------
echo "==> step 0: open62541 client <-> open62541 server"
stdbuf -oL -eL "$EX/tutorial_server_variable" > "$LOGDIR/server0.log" 2>&1 &
SRV=$!
sleep 3
if ! kill -0 "$SRV" 2>/dev/null; then
  echo "PROOF-FAIL: the foreign server died before the foreign client ran"
  tail -20 "$LOGDIR/server0.log"
  exit 1
fi
stdbuf -oL -eL "$EX/tutorial_client_firststeps" > "$LOGDIR/client0.log" 2>&1
FOREIGN=$?
kill -INT "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null; SRV=
if [ "$FOREIGN" -ne 0 ]; then
  echo "PROOF-FAIL: the foreign round trip itself failed (exit $FOREIGN)"
  tail -20 "$LOGDIR/client0.log"
  exit 1
fi
echo "    the counterpart is alive"

# --- Step 1: the ZeroDDS client against the same foreign server. ------------
echo "==> step 1: ZeroDDS client -> open62541 server"
cargo build -q -p zerodds-opcua-server --example open62541_probe || exit 1
stdbuf -oL -eL "$EX/tutorial_server_variable" > "$LOGDIR/server1.log" 2>&1 &
SRV=$!
sleep 3
"$REPO/target/debug/examples/open62541_probe" "127.0.0.1:$PORT"
PROBE=$?
kill -INT "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null; SRV=

if [ "$PROBE" -eq 0 ]; then
  echo "PROOF-PASS: GetEndpoints, CreateSession, ActivateSession, Read, Write"
  echo "            and Browse answered by open62541"
  exit 0
fi
echo "PROOF-FAIL: the ZeroDDS client did not complete against open62541 (exit $PROBE)"
echo "--- foreign server log ---"
grep -Ei "warn|error|unknown request" "$LOGDIR/server1.log" | tail -20
exit 1
