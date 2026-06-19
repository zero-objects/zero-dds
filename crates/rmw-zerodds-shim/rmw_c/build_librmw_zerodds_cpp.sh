#!/usr/bin/env bash
# Build the ABI-correct rmw layer librmw_zerodds_cpp.so from rmw_zerodds.c,
# against an installed ROS 2 Humble (dev headers), linking the Rust DDS bridge
# (librmw_zerodds.so) + the ZeroDDS C-API (libzerodds.so).
#
# Prereqs:
#   * A ROS 2 Humble install with dev headers (e.g. RoboStack micromamba env, or
#     a system /opt/ros/humble). Point $ROS_PREFIX at it ($CONDA_PREFIX or
#     /opt/ros/humble).
#   * `cargo build -p rmw-zerodds-shim --release` has produced
#     $ZERODDS_TARGET/librmw_zerodds.so + libzerodds.so.
#
# Usage:
#   ROS_PREFIX=$CONDA_PREFIX ZERODDS_TARGET=/path/to/zerodds/target/release \
#     ./build_librmw_zerodds_cpp.sh
#
# Notes:
#   * `-idirafter` (not `-I`) for the ROS include dirs so the SYSTEM libc headers
#     win — some ROS deps ship a clashing top-level header (CycloneDDS'
#     dds/features.h shadows glibc <features.h> under a plain -I).
#   * ROS 2 headers are double-nested ($PREFIX/include/<pkg>/<pkg>/h), so each
#     package dir needs its own include flag.
set -euo pipefail

ROS_PREFIX="${ROS_PREFIX:?set ROS_PREFIX to the ROS 2 install prefix}"
ZERODDS_TARGET="${ZERODDS_TARGET:?set ZERODDS_TARGET to the cargo release dir}"
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${OUT:-$HERE/librmw_zerodds_cpp.so}"

INC=""
for d in "$ROS_PREFIX"/include/*/; do INC="$INC -idirafter $d"; done

cc -shared -fPIC -o "$OUT" "$HERE/rmw_zerodds.c" \
  $INC -idirafter "$ROS_PREFIX/include" \
  -L"$ZERODDS_TARGET" -L"$ROS_PREFIX/lib" \
  -Wl,-rpath,"$ZERODDS_TARGET" -Wl,-rpath,"$ROS_PREFIX/lib" \
  -lrmw_zerodds -lzerodds -lrmw -lrcutils \
  -lrosidl_typesupport_introspection_c -lrosidl_runtime_c

echo "built $OUT"
echo "register: cp '$OUT' '$ROS_PREFIX/lib/' && \\"
echo "  mkdir -p '$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation' && \\"
echo "  touch '$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation/rmw_zerodds_cpp'"
echo "use: RMW_IMPLEMENTATION=rmw_zerodds_cpp <ros2 command>"
