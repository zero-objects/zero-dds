#!/usr/bin/env bash
# Build the ABI-correct rmw layer, register it into a ROS 2 install, then call
# the demo_nodes_py AddTwoInts service across RMWs: a server under one
# RMW_IMPLEMENTATION, a client under another, the client must print the sum.
#
# Pairs with rmw_zerodds_cpp on either side against rmw_cyclonedds_cpp and
# rmw_fastrtps_cpp, and zerodds with itself, must pass. The graph runs check
# that the service, its type and the server node's services are reported
# across RMWs (`ros2 service list -t`, `ros2 node info`). The pair
# cyclonedds <-> fastrtps runs as a reference and is reported, not judged:
# the two do not exchange service calls (each correlates in its own form).
#
# Usage:
#   ROS_PREFIX=$CONDA_PREFIX ./run_ros2_services_test.sh
#
# Env knobs: REPO (repo root, auto-detected), ZERODDS_TARGET (cargo release
# dir), ROS_DOMAIN_ID (default 49), CALL_TIMEOUT (seconds, default 30).
# NB: no `set -u` — ROS's setup.bash references unset vars.
set -eo pipefail

: "${ROS_PREFIX:?set ROS_PREFIX to the ROS 2 install prefix (e.g. \$CONDA_PREFIX or /opt/ros/humble)}"
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="${REPO:-$(cd "$HERE/../../.." && pwd)}"
ZERODDS_TARGET="${ZERODDS_TARGET:-$REPO/target/release}"

set +e
# shellcheck disable=SC1091
source "$ROS_PREFIX/setup.bash" >/dev/null 2>&1 || true
set -e

echo "== 1) Rust DDS bridge + ZeroDDS C-API =="
( cd "$REPO" && cargo build -p rmw-zerodds-shim -p zerodds-c-api --release )

echo "== 2) ABI-correct rmw layer (librmw_zerodds_cpp.so) =="
ROS_PREFIX="$ROS_PREFIX" ZERODDS_TARGET="$ZERODDS_TARGET" \
  bash "$HERE/build_librmw_zerodds_cpp.sh"

echo "== 3) register into the ROS install =="
cp "$HERE/librmw_zerodds_cpp.so" "$ROS_PREFIX/lib/"
for _lib in librmw_zerodds.so libzerodds.so; do
  [ -f "$ZERODDS_TARGET/$_lib" ] && cp "$ZERODDS_TARGET/$_lib" "$ROS_PREFIX/lib/"
done
mkdir -p "$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation"
touch "$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation/rmw_zerodds_cpp"

export LD_LIBRARY_PATH="$ZERODDS_TARGET:$ROS_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-49}"
DEMO="$ROS_PREFIX/lib/demo_nodes_py"
TIMEOUT="${CALL_TIMEOUT:-30}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
rc=0

# call SERVER_RMW CLIENT_RMW judge|reference: one service call.
call() {
  local label="server $1, client $2"
  echo "== $label =="
  RMW_IMPLEMENTATION="$1" python "$DEMO/add_two_ints_server" > "$TMP/server.log" 2>&1 &
  local server=$!
  sleep 3
  local ok=0
  if RMW_IMPLEMENTATION="$2" timeout "$TIMEOUT" python "$DEMO/add_two_ints_client" \
      > "$TMP/client.log" 2>&1 && grep -q "Result of add_two_ints: 5" "$TMP/client.log"; then
    ok=1
  fi
  kill "$server" 2>/dev/null || true
  wait "$server" 2>/dev/null || true
  echo "-- server --"; cat "$TMP/server.log"
  echo "-- client --"; cat "$TMP/client.log"
  if [ "$ok" = 1 ]; then
    echo "RESULT $label: pass"
  elif [ "$3" = judge ]; then
    echo "RESULT $label: FAIL"; rc=1
  else
    echo "RESULT $label: no call (reference)"
  fi
}

# graph SERVER_RMW QUERY_RMW: the service and its type as `ros2 service list
# -t` and the server node's `ros2 node info` report them under QUERY_RMW (both
# without the daemon, which would answer from whichever RMW started it).
graph() {
  local label="graph: server $1, query $2"
  echo "== $label =="
  RMW_IMPLEMENTATION="$1" python "$DEMO/add_two_ints_server" > "$TMP/server.log" 2>&1 &
  local server=$!
  sleep 4
  local ok=1
  RMW_IMPLEMENTATION="$2" timeout 20 ros2 service list --no-daemon -t \
    > "$TMP/type.log" 2>&1 || true
  RMW_IMPLEMENTATION="$2" timeout 20 ros2 node info --no-daemon /add_two_ints_server \
    > "$TMP/node.log" 2>&1 || true
  kill "$server" 2>/dev/null || true
  wait "$server" 2>/dev/null || true
  echo "-- ros2 service list -t --"; cat "$TMP/type.log"
  echo "-- ros2 node info --"; cat "$TMP/node.log"
  grep -qx "/add_two_ints \[example_interfaces/srv/AddTwoInts\]" "$TMP/type.log" || ok=0
  awk '/Service Servers:/{f=1;next} /Service Clients:|Action/{f=0} f' "$TMP/node.log" \
    | grep -q "/add_two_ints: example_interfaces/srv/AddTwoInts" || ok=0
  if [ "$ok" = 1 ]; then echo "RESULT $label: pass"; else echo "RESULT $label: FAIL"; rc=1; fi
}

call rmw_zerodds_cpp rmw_zerodds_cpp judge
call rmw_cyclonedds_cpp rmw_zerodds_cpp judge
call rmw_zerodds_cpp rmw_cyclonedds_cpp judge
call rmw_fastrtps_cpp rmw_zerodds_cpp judge
call rmw_zerodds_cpp rmw_fastrtps_cpp judge
call rmw_cyclonedds_cpp rmw_fastrtps_cpp reference
call rmw_fastrtps_cpp rmw_cyclonedds_cpp reference
graph rmw_zerodds_cpp rmw_zerodds_cpp
graph rmw_zerodds_cpp rmw_cyclonedds_cpp
graph rmw_zerodds_cpp rmw_fastrtps_cpp
graph rmw_cyclonedds_cpp rmw_zerodds_cpp
graph rmw_fastrtps_cpp rmw_zerodds_cpp
exit "$rc"
