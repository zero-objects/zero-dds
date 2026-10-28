#!/usr/bin/env bash
# Build the ABI-correct rmw layer, register it into a ROS 2 install, then build
# and run the rclcpp ros2-config e2e test (ros2_config_test.cpp): the ros2
# section of the runtime configuration (topic_mangling none, a topic override)
# must reach the endpoints the RMW creates.
#
# The RMW reads the layers every ZeroDDS program reads: environment, the
# files (`--config` on the node's command line, else the system directory),
# `--set` on the command line (outside `--ros-args … --`). The run installs
# one drop-in, $ETC/zerodds.d/90-ros2-config-test.yaml, for the runs that
# need it and removes it on every exit; the `--config`/`--set` runs use no
# drop-in. It refuses to start when that file exists. Only the RMW reads
# ros2.*; other programs on the host leave it alone.
#
# Usage (as root, to write $ETC):
#   ROS_PREFIX=$CONDA_PREFIX ./run_ros2_config_test.sh
#
# Env knobs: REPO (repo root, auto-detected), ZERODDS_TARGET (cargo release
# dir), ROS_DOMAIN_ID (default 48), RMW_OTHER (default rmw_cyclonedds_cpp; the
# RMW that reads the DDS topic names off the wire and subscribes under
# avoid_ros_namespace_conventions).
# NB: no `set -u` — ROS's setup.bash references unset vars.
set -eo pipefail

: "${ROS_PREFIX:?set ROS_PREFIX to the ROS 2 install prefix (e.g. \$CONDA_PREFIX or /opt/ros/humble)}"
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="${REPO:-$(cd "$HERE/../../.." && pwd)}"
ZERODDS_TARGET="${ZERODDS_TARGET:-$REPO/target/release}"
ETC=/etc/zerodds

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

echo "== 4) build the rclcpp ros2-config test (ament/CMake) =="
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp "$HERE/ros2_config_test.cpp" "$TMP/"
cat > "$TMP/CMakeLists.txt" <<'CMAKE'
cmake_minimum_required(VERSION 3.8)
project(zerodds_ros2_config_test)
find_package(rclcpp REQUIRED)
find_package(std_msgs REQUIRED)
add_executable(ros2_config_test ros2_config_test.cpp)
ament_target_dependencies(ros2_config_test rclcpp std_msgs)
CMAKE
cmake -S "$TMP" -B "$TMP/build" -DCMAKE_PREFIX_PATH="$ROS_PREFIX" >/dev/null
cmake --build "$TMP/build" >/dev/null

FRAGMENT="$ETC/zerodds.d/90-ros2-config-test.yaml"
if [ -e "$FRAGMENT" ]; then
  echo "refusing: $FRAGMENT exists (another run, or left behind)" >&2
  exit 2
fi
trap 'rm -f "$FRAGMENT"; rm -rf "$TMP"' EXIT
cat > "$TMP/fragment.yaml" <<'YAML'
ros2:
  topic_mangling: none
  topic_overrides:
    - ros_topic: /zerodds_cfg_topic
      qos:
        durability: transient_local
        history: keep_last
        depth: 1
YAML
# The override alone: topic_mangling stays at its default (rmw), so only a
# --set can make the DDS topic bare.
cat > "$TMP/override-only.yaml" <<'YAML'
ros2:
  topic_overrides:
    - ros_topic: /zerodds_cfg_topic
      qos:
        durability: transient_local
        history: keep_last
        depth: 1
YAML
mkdir -p "$ETC/zerodds.d"

# with_config on|off: the drop-in present or absent for the next pair.
with_config() {
  if [ "$1" = on ]; then cp "$TMP/fragment.yaml" "$FRAGMENT"; else rm -f "$FRAGMENT"; fi
}

# run_as RMW MODE [ARG]: one test process.
run_as() {
  RMW_IMPLEMENTATION="$1" "$TMP/build/ros2_config_test" "${@:2}"
}

export LD_LIBRARY_PATH="$ZERODDS_TARGET:$ROS_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-48}"
OTHER="${RMW_OTHER:-rmw_cyclonedds_cpp}"
rc=0

# pair PUB_MODE LABEL EXPECT(pass|fail) SUB_RMW SUB_MODE [ARG]: a zerodds
# publisher, then the subscriber side in a second process.
pair() {
  echo "== $2 =="
  # shellcheck disable=SC2086  # PUB_ARGS: the publisher's --config/--set, split on purpose
  run_as rmw_zerodds_cpp "$1" ${PUB_ARGS:-} > "$TMP/pub.log" 2>&1 &
  local pub=$!
  sleep 2
  local sub_rc=0
  run_as "${@:4}" || sub_rc=1
  local pub_rc=0
  wait "$pub" || pub_rc=1
  echo "-- publisher --"
  cat "$TMP/pub.log"
  if [ "$3" = pass ] && { [ "$sub_rc" != 0 ] || [ "$pub_rc" != 0 ]; }; then rc=1; fi
  if [ "$3" = fail ] && [ "$sub_rc" = 0 ]; then
    echo "Gegenprobe: the check passed without the configuration"; rc=1
  fi
}

with_config on
pair pub "5a) override on both sides: zerodds -> zerodds" pass \
  rmw_zerodds_cpp sub
pair pub "5b) topic_mangling none on the wire: $OTHER sees the DDS topic zerodds_cfg_topic" pass \
  "$OTHER" list-raw zerodds_cfg_topic
with_config off
pair pub "5c) Gegenprobe without the configuration: no bare zerodds_cfg_topic" fail \
  "$OTHER" list-raw zerodds_cfg_topic
pair pub-avoid "5d) avoid_ros_namespace_conventions: zerodds -> $OTHER (/zerodds_cfg_topic)" pass \
  "$OTHER" sub-avoid
# The same section from the node's command line, no drop-in.
PUB_ARGS="--config $TMP/fragment.yaml" \
  pair pub "5e) --config on the node's command line: $OTHER sees zerodds_cfg_topic" pass \
  "$OTHER" list-raw zerodds_cfg_topic
PUB_ARGS="--config $TMP/override-only.yaml --ros-args -r __ns:=/ -- --set ros2.topic_mangling=none" \
  pair pub "5f) --set after the --ros-args block, over the file: $OTHER sees zerodds_cfg_topic" pass \
  "$OTHER" list-raw zerodds_cfg_topic
PUB_ARGS="--config $TMP/fragment.yaml --set ros2.topic_mangling=rmw" \
  pair pub "5g) Gegenprobe: --set wins over --config, no bare zerodds_cfg_topic" fail \
  "$OTHER" list-raw zerodds_cfg_topic
exit "$rc"
