#!/usr/bin/env bash
# Build the ABI-correct rmw layer, register it into a ROS 2 install, then build
# and run the REP-2007 type-adaptation e2e test (type_adaptation_test.cpp).
#
# REP-2007 places the TypeAdapter template in rclcpp; the middleware half it
# named ("custom type support, which may be middleware specific") lives in
# rmw_zerodds_ros2_ext.h and is what this test exercises — through the rmw
# handles of live rclcpp entities, so the measurement is against the real
# loaded rmw implementation, not a mock.
#
# Usage:
#   ROS_PREFIX=$CONDA_PREFIX ./run_type_adaptation_test.sh
#   ROS_PREFIX=/opt/ros/humble ./run_type_adaptation_test.sh
#
# Env knobs: REPO (repo root, auto-detected), ZERODDS_TARGET (cargo release dir),
# ROS_DOMAIN_ID (default 46).
# NB: no `set -u` — ROS's setup.bash references unset vars and would abort the
# shell under nounset (the `|| true` does not catch a nounset-fatal).
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

echo "== 4) build the REP-2007 type-adaptation test (ament/CMake) =="
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp "$HERE/type_adaptation_test.cpp" "$HERE/rmw_zerodds_ros2_ext.h" "$TMP/"
cat > "$TMP/CMakeLists.txt" <<'CMAKE'
cmake_minimum_required(VERSION 3.8)
project(zerodds_type_adaptation_test)
find_package(rclcpp REQUIRED)
find_package(std_msgs REQUIRED)
find_package(example_interfaces REQUIRED)
add_executable(type_adaptation_test type_adaptation_test.cpp)
target_include_directories(type_adaptation_test PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})
ament_target_dependencies(type_adaptation_test rclcpp std_msgs example_interfaces)
# The extension surface is exported by the rmw implementation itself. Link it
# directly: at runtime rmw_implementation dlopens the same SONAME, so both
# references resolve to one loaded library and one set of entity data.
find_library(RMW_ZERODDS_CPP rmw_zerodds_cpp HINTS "$ENV{ROS_PREFIX}/lib" REQUIRED)
target_link_libraries(type_adaptation_test ${RMW_ZERODDS_CPP})
CMAKE
ROS_PREFIX="$ROS_PREFIX" cmake -S "$TMP" -B "$TMP/build" -DCMAKE_PREFIX_PATH="$ROS_PREFIX" >/dev/null
cmake --build "$TMP/build" >/dev/null

echo "== 5) run over rmw_zerodds_cpp =="
export RMW_IMPLEMENTATION=rmw_zerodds_cpp
export LD_LIBRARY_PATH="$ZERODDS_TARGET:$ROS_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-46}" "$TMP/build/type_adaptation_test"
