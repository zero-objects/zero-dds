#!/usr/bin/env bash
# Build the ABI-correct rmw layer, register it into a ROS 2 install, then build
# and run the rclcpp QoS-translation e2e test (qos_translation_test.cpp): a
# TRANSIENT_LOCAL KEEP_LAST(1) publisher in one process, a late-joining
# subscription in a second, which must receive exactly the newest sample, and
# rmw_*_get_actual_qos reporting what was applied.
#
# Usage:
#   ROS_PREFIX=$CONDA_PREFIX ./run_qos_translation_test.sh
#
# Env knobs: REPO (repo root, auto-detected), ZERODDS_TARGET (cargo release dir),
# ROS_DOMAIN_ID (default 47), RMW_UNDER_TEST (default rmw_zerodds_cpp; a
# reference RMW such as rmw_cyclonedds_cpp checks the test itself), RMW_PUB /
# RMW_SUB (one RMW per process, for the cross-vendor direction).
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
# Refresh the Rust bridge + C-API inside the prefix too. An ament install may
# have left older copies of librmw_zerodds.so / libzerodds.so there; with the
# prefix on LD_LIBRARY_PATH those would be loaded instead of the build under
# test, and the run would measure stale code without saying so. $ZERODDS_TARGET
# also comes first on LD_LIBRARY_PATH below, so the fresh build wins twice.
for _lib in librmw_zerodds.so libzerodds.so; do
  [ -f "$ZERODDS_TARGET/$_lib" ] && cp "$ZERODDS_TARGET/$_lib" "$ROS_PREFIX/lib/"
done
mkdir -p "$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation"
touch "$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation/rmw_zerodds_cpp"

echo "== 4) build the rclcpp qos-translation test (ament/CMake) =="
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp "$HERE/qos_translation_test.cpp" "$TMP/"
cat > "$TMP/CMakeLists.txt" <<'CMAKE'
cmake_minimum_required(VERSION 3.8)
project(zerodds_qos_translation_test)
find_package(rclcpp REQUIRED)
find_package(std_msgs REQUIRED)
add_executable(qos_translation_test qos_translation_test.cpp)
ament_target_dependencies(qos_translation_test rclcpp std_msgs)
CMAKE
cmake -S "$TMP" -B "$TMP/build" -DCMAKE_PREFIX_PATH="$ROS_PREFIX" >/dev/null
cmake --build "$TMP/build" >/dev/null

echo "== 5) run over rmw_zerodds_cpp: publisher first, late joiner in a second process =="
export RMW_IMPLEMENTATION="${RMW_UNDER_TEST:-rmw_zerodds_cpp}"
export LD_LIBRARY_PATH="$ZERODDS_TARGET:$ROS_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-47}"
RMW_IMPLEMENTATION="${RMW_PUB:-$RMW_IMPLEMENTATION}" \
  "$TMP/build/qos_translation_test" pub > "$TMP/pub.log" 2>&1 &
PUB=$!
sleep 2
rc=0
RMW_IMPLEMENTATION="${RMW_SUB:-$RMW_IMPLEMENTATION}" \
  "$TMP/build/qos_translation_test" sub || rc=1
wait "$PUB" || rc=1
echo "-- publisher --"
cat "$TMP/pub.log"
exit "$rc"
