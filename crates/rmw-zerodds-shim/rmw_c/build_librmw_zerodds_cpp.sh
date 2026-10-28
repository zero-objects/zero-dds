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

# rmw_zerodds_cxx.cpp holds the std::string / std::u16string half of the
# introspection codec: rosidl's C++ introspection typesupport (what rclcpp
# resolves to) describes std::string members, which a C translation unit cannot
# touch. Compiled separately and linked in; -lstdc++ for its runtime.
CXX_OBJ="$(mktemp -d)/rmw_zerodds_cxx.o"
trap 'rm -rf "$(dirname "$CXX_OBJ")"' EXIT
c++ -c -fPIC -o "$CXX_OBJ" "$HERE/rmw_zerodds_cxx.cpp"
# rmw_zerodds_qos.cpp: rmw_qos_profile_check_compatible through rmw_dds_common,
# as the other RMWs check it.
QOS_OBJ="$(dirname "$CXX_OBJ")/rmw_zerodds_qos.o"
c++ -c -fPIC -std=c++17 -o "$QOS_OBJ" "$HERE/rmw_zerodds_qos.cpp" \
  $INC -idirafter "$ROS_PREFIX/include"

# rmw_zerodds_dynamic.cpp: ROS 2 Jazzy's dynamic-type-support backend
# (rosidl_dynamic_typesupport_serialization_support_interface_t +
# rmw_serialization_support_init/rmw_take_dynamic_message[_with_info]).
# Guarded on `__has_include(<rosidl_dynamic_typesupport/types.h>)`, which
# Humble does not have — this compiles to an empty translation unit
# there, so building it unconditionally here costs nothing on Humble and
# needs nothing extra on Jazzy beyond the include path this script
# already builds from $ROS_PREFIX.
DYNAMIC_OBJ="$(dirname "$CXX_OBJ")/rmw_zerodds_dynamic.o"
c++ -c -fPIC -std=c++17 -o "$DYNAMIC_OBJ" "$HERE/rmw_zerodds_dynamic.cpp" \
  $INC -idirafter "$ROS_PREFIX/include"

cc -shared -fPIC -o "$OUT" "$HERE/rmw_zerodds.c" "$CXX_OBJ" "$QOS_OBJ" "$DYNAMIC_OBJ" \
  $INC -idirafter "$ROS_PREFIX/include" \
  -L"$ZERODDS_TARGET" -L"$ROS_PREFIX/lib" \
  -Wl,-rpath,"$ZERODDS_TARGET" -Wl,-rpath,"$ROS_PREFIX/lib" \
  -lrmw_zerodds -lzerodds -lrmw -lrmw_dds_common -lrcutils \
  -lrosidl_typesupport_introspection_c -lrosidl_runtime_c -lstdc++

echo "built $OUT"
echo "register: cp '$OUT' '$ROS_PREFIX/lib/' && \\"
echo "  mkdir -p '$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation' && \\"
echo "  touch '$ROS_PREFIX/share/ament_index/resource_index/rmw_implementation/rmw_zerodds_cpp'"
echo "use: RMW_IMPLEMENTATION=rmw_zerodds_cpp <ros2 command>"
