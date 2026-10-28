#!/usr/bin/env bash
# Build the ABI-correct rmw layer, register it into a ROS 2 Jazzy install,
# then build and run the dynamic-type-support e2e test
# (dynamic_typesupport_test.cpp): std_msgs/msg/String and
# test_msgs/msg/BasicTypes, taken through rmw_take_dynamic_message against
# this backend's own rmw_serialization_support_init +
# dynamic_type_builder_* + dynamic_data_* calls, and compared against a
# plain rclcpp::Subscription on the same topic.
#
# ROS 2 Jazzy only: rosidl_dynamic_typesupport does not exist on Humble,
# and this script's own `find_package(rosidl_dynamic_typesupport REQUIRED)`
# fails loudly there rather than silently skipping the test.
#
# Usage:
#   ROS_PREFIX=/opt/ros/jazzy ./run_dynamic_typesupport_test.sh
#
# Env knobs: REPO (repo root, auto-detected), ZERODDS_TARGET (cargo release
# dir), ROS_DOMAIN_ID (default 47).
# NB: no `set -u` — ROS's setup.bash references unset vars.
set -eo pipefail

: "${ROS_PREFIX:?set ROS_PREFIX to a ROS 2 Jazzy install prefix (e.g. /opt/ros/jazzy)}"
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

echo "== 4) build the dynamic-type-support test (ament/CMake) =="
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp "$HERE/dynamic_typesupport_test.cpp" "$TMP/"
cat > "$TMP/CMakeLists.txt" <<'CMAKE'
cmake_minimum_required(VERSION 3.8)
project(zerodds_dynamic_typesupport_test)
find_package(rclcpp REQUIRED)
find_package(std_msgs REQUIRED)
find_package(test_msgs REQUIRED)
find_package(rosidl_dynamic_typesupport REQUIRED)
add_executable(dynamic_typesupport_test dynamic_typesupport_test.cpp)
ament_target_dependencies(dynamic_typesupport_test
  rclcpp std_msgs test_msgs rosidl_dynamic_typesupport)
find_library(RMW_ZERODDS_CPP rmw_zerodds_cpp HINTS "$ENV{ROS_PREFIX}/lib" REQUIRED)
target_link_libraries(dynamic_typesupport_test ${RMW_ZERODDS_CPP})
CMAKE
ROS_PREFIX="$ROS_PREFIX" cmake -S "$TMP" -B "$TMP/build" -DCMAKE_PREFIX_PATH="$ROS_PREFIX" >/dev/null
cmake --build "$TMP/build" >/dev/null

echo "== 5) run over rmw_zerodds_cpp =="
export RMW_IMPLEMENTATION=rmw_zerodds_cpp
export LD_LIBRARY_PATH="$ZERODDS_TARGET:$ROS_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-47}" "$TMP/build/dynamic_typesupport_test"
