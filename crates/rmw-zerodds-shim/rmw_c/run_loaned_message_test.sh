#!/usr/bin/env bash
# Build the ABI-correct rmw layer, register it into a ROS 2 install, then build
# and run the rclcpp loaned-message e2e test (loaned_message_test.cpp).
#
# rclpy exposes no loaned-message API, so this rclcpp (C++) test is the only way
# to exercise rmw_borrow/publish/take_loaned_message + can_loan_messages.
# A throwaway ament/CMake project resolves rclcpp's transitive link deps (far
# more robust than a hand-written g++ link line).
#
# Verified green on codepit (ROS 2 Humble via RoboStack/micromamba):
#   can_loan=1 got=42 PASS
#
# Usage:
#   ROS_PREFIX=$CONDA_PREFIX ./run_loaned_message_test.sh
#
# Env knobs: REPO (repo root, auto-detected), ZERODDS_TARGET (cargo release dir),
# ROS_DOMAIN_ID (default 44).
# NB: no `set -u` — ROS's setup.bash references unset vars and would abort the
# shell under nounset (the `|| true` does not catch a nounset-fatal).
set -eo pipefail

: "${ROS_PREFIX:?set ROS_PREFIX to the ROS 2 install prefix (e.g. \$CONDA_PREFIX or /opt/ros/humble)}"
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="${REPO:-$(cd "$HERE/../../.." && pwd)}"
ZERODDS_TARGET="${ZERODDS_TARGET:-$REPO/target/release}"

# Make ROS available; relax errexit around the (not errexit-clean) setup script.
set +e
# shellcheck disable=SC1091
source "$ROS_PREFIX/setup.bash" >/dev/null 2>&1 || true
set -e

echo "== 1) Rust DDS bridge + ZeroDDS C-API =="
# ZERODDS_TEST_ICEORYX=1 also builds the iceoryx2 backend (delivery mode
# `Iceoryx`) and runs that mode; the default run stays lean (Portable + SHM).
CARGO_FEAT=""
if [ "${ZERODDS_TEST_ICEORYX:-0}" = "1" ]; then
  CARGO_FEAT="--features rmw-zerodds-shim/delivery-iceoryx"
fi
( cd "$REPO" && cargo build -p rmw-zerodds-shim -p zerodds-c-api --release $CARGO_FEAT )

echo "== 2) ABI-correct rmw layer (librmw_zerodds_cpp.so) =="
ROS_PREFIX="$ROS_PREFIX" ZERODDS_TARGET="$ZERODDS_TARGET" \
  bash "$HERE/build_librmw_zerodds_cpp.sh"

echo "== 3) register into the ROS install =="
cp "$HERE/librmw_zerodds_cpp.so" "$ROS_PREFIX/lib/"
mkdir -p "$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation"
touch "$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation/rmw_zerodds_cpp"

echo "== 4) build the rclcpp loaned-message test (ament/CMake) =="
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp "$HERE/loaned_message_test.cpp" "$TMP/"
cat > "$TMP/CMakeLists.txt" <<'CMAKE'
cmake_minimum_required(VERSION 3.8)
project(zerodds_loaned_message_test)
find_package(rclcpp REQUIRED)
find_package(std_msgs REQUIRED)
add_executable(loaned_message_test loaned_message_test.cpp)
ament_target_dependencies(loaned_message_test rclcpp std_msgs)
CMAKE
cmake -S "$TMP" -B "$TMP/build" -DCMAKE_PREFIX_PATH="$ROS_PREFIX" >/dev/null
cmake --build "$TMP/build" >/dev/null

echo "== 5) run over rmw_zerodds_cpp (both delivery modes) =="
export RMW_IMPLEMENTATION=rmw_zerodds_cpp
export LD_LIBRARY_PATH="$ROS_PREFIX/lib:$ZERODDS_TARGET${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
rc=0

# Portable (default): serialize struct→CDR at publish, normal RTPS take.
echo "-- mode: portable (default) --"
ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-44}" "$TMP/build/loaned_message_test" || rc=1

# RawSameHost: same-host zero-copy SHM, no wire. Because the raw writer never
# publishes over RTPS, a delivered value proves the SHM path.
echo "-- mode: raw-same-host --"
rm -f /tmp/zerodds/rmw_*.flink 2>/dev/null || true
ZERODDS_DELIVERY_MODE=raw-same-host ROS_DOMAIN_ID="$(( ${ROS_DOMAIN_ID:-44} + 1 ))" \
  "$TMP/build/loaned_message_test" || rc=1

# Iceoryx: same-host cross-stack over iceoryx2 (only when built with the feature).
if [ "${ZERODDS_TEST_ICEORYX:-0}" = "1" ]; then
  echo "-- mode: iceoryx --"
  ZERODDS_DELIVERY_MODE=iceoryx ROS_DOMAIN_ID="$(( ${ROS_DOMAIN_ID:-44} + 2 ))" \
    "$TMP/build/loaned_message_test" || rc=1
fi

exit "$rc"
