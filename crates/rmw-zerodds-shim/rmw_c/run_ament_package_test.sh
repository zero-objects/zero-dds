#!/usr/bin/env bash
# Build the ament package crates/rmw-zerodds-shim/ament (rmw_zerodds_cpp) the
# way colcon does — CMake configure, build, install — into a scratch prefix,
# and check what an installed RMW needs: the library RMW_IMPLEMENTATION loads,
# the two libraries it depends on by name next to it, the ament index
# entries, and that rclpy loads this copy and no other.
#
# Usage:
#   ROS_PREFIX=$CONDA_PREFIX ./run_ament_package_test.sh
set -eo pipefail

: "${ROS_PREFIX:?set ROS_PREFIX to the ROS 2 install prefix (e.g. \$CONDA_PREFIX or /opt/ros/humble)}"
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="${REPO:-$(cd "$HERE/../../.." && pwd)}"
PYTHON="${PYTHON:-python}"

set +e
source "$ROS_PREFIX/setup.bash" >/dev/null 2>&1 || true
set -e

echo "== 1) Rust DDS bridge + ZeroDDS C-API =="
( cd "$REPO" && cargo build -p rmw-zerodds-shim -p zerodds-c-api --release )

echo "== 2) configure, build, install the ament package =="
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
ZERODDS_HOME="$REPO" cmake -S "$REPO/crates/rmw-zerodds-shim/ament" -B "$TMP/build" \
  -DCMAKE_PREFIX_PATH="$ROS_PREFIX" -DCMAKE_INSTALL_PREFIX="$TMP/install" >/dev/null
cmake --build "$TMP/build" >/dev/null
cmake --install "$TMP/build" >/dev/null

rc=0
check() { if eval "$2"; then echo "ok   $1"; else echo "FAIL $1"; rc=1; fi; }
LIB="$TMP/install/lib/librmw_zerodds_cpp.so"
check "lib/librmw_zerodds_cpp.so installed" "[ -f '$LIB' ]"
check "lib/librmw_zerodds.so and lib/libzerodds.so installed" \
  "[ -f '$TMP/install/lib/librmw_zerodds.so' ] && [ -f '$TMP/install/lib/libzerodds.so' ]"
check "the dependencies are named, not pathed" \
  "readelf -d '$LIB' | grep NEEDED | grep -q '\[librmw_zerodds.so\]' && ! readelf -d '$LIB' | grep NEEDED | grep -q '/'"
IDX="$TMP/install/share/ament_index/resource_index"
check "ament index: rmw_typesupport/rmw_zerodds_cpp" "[ -f '$IDX/rmw_typesupport/rmw_zerodds_cpp' ]"
check "ament index: packages/rmw_zerodds_cpp" "[ -f '$IDX/packages/rmw_zerodds_cpp' ]"

echo "== 3) rclpy loads the installed copy =="
# The other runners copy librmw_zerodds_cpp.so into $ROS_PREFIX/lib, and the
# ROS libraries' own search path finds that copy first. It is set aside for
# this check and put back on every exit.
PREFIX_COPY="$ROS_PREFIX/lib/librmw_zerodds_cpp.so"
if [ -e "$PREFIX_COPY" ]; then
  mv "$PREFIX_COPY" "$PREFIX_COPY.ament-test"
  trap 'mv "$PREFIX_COPY.ament-test" "$PREFIX_COPY"; rm -rf "$TMP"' EXIT
fi
LOADED="$(AMENT_PREFIX_PATH="$TMP/install:${AMENT_PREFIX_PATH:-}" \
  LD_LIBRARY_PATH="$TMP/install/lib:$ROS_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  RMW_IMPLEMENTATION=rmw_zerodds_cpp ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-44}" \
  "$PYTHON" -c '
import rclpy
rclpy.init()
node = rclpy.create_node("zerodds_ament_probe")
print([l.split()[-1] for l in open("/proc/self/maps") if "librmw_zerodds_cpp" in l][0])
node.destroy_node()
rclpy.shutdown()
' 2>&1 | tail -1)"
check "rclpy loaded $TMP/install/lib/librmw_zerodds_cpp.so (loaded: $LOADED)" \
  "[ '$LOADED' = '$LIB' ]"
exit "$rc"
