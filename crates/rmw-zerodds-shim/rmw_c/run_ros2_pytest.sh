#!/usr/bin/env bash
# Reproducible §6.4 runner (zerodds-py-1.0): build the ABI-correct rmw layer,
# register it into a ROS 2 install, and run the zerodds-py ROS-2 interop pytest
# (rclpy over ZeroDDS) against it.
#
# Verified green on the Linux test host (ROS 2 Humble via RoboStack/micromamba), 2026-06-13:
#   test_rclpy_init_succeeds_with_zerodds_rmw            PASSED
#   test_rclpy_publish_subscribe_string_roundtrip        PASSED  (std_msgs/String)
#
# IMPORTANT: `cargo build -p rmw-zerodds-shim` alone produces ONLY the prefixed
# `librmw_zerodds.so` (rmw_zerodds_* symbols). Stock rclpy's rmw_implementation
# loads `librmw_zerodds_cpp.so` with the UNPREFIXED `rmw_*` surface — that is the
# C layer (rmw_zerodds.c) built by build_librmw_zerodds_cpp.sh. This runner does
# both.
#
# Usage:
#   ROS_PREFIX=$CONDA_PREFIX ./run_ros2_pytest.sh            # RoboStack env
#   ROS_PREFIX=/opt/ros/humble ./run_ros2_pytest.sh          # system install
#
# Env knobs: REPO (repo root, auto-detected), ZERODDS_TARGET (cargo release dir),
# ROS_DOMAIN_ID (default 42), PYTHON (default: python on PATH).
set -euo pipefail

: "${ROS_PREFIX:?set ROS_PREFIX to the ROS 2 install prefix (e.g. \$CONDA_PREFIX or /opt/ros/humble)}"
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="${REPO:-$(cd "$HERE/../../.." && pwd)}"
ZERODDS_TARGET="${ZERODDS_TARGET:-$REPO/target/release}"
PYTHON="${PYTHON:-python}"

# Make ROS available (ament prefix paths, rclpy). Non-fatal if already sourced.
# ROS's setup.bash references unbound vars, so disable `set -u` around it — under
# nounset the unbound reference aborts the shell immediately and `|| true` cannot
# catch it.
set +u
# shellcheck disable=SC1091
source "$ROS_PREFIX/setup.bash" 2>/dev/null || true
set -u

echo "== 1) Rust DDS bridge + ZeroDDS C-API =="
# Build BOTH the shim cdylib (librmw_zerodds.so) AND the ZeroDDS C-API cdylib
# (libzerodds.so) — the C layer links `-lzerodds`, and `-p rmw-zerodds-shim`
# alone only links the c-api rlib into the shim, it does not emit libzerodds.so
# (a fresh checkout would fail the link with `cannot find -lzerodds`).
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

echo "== 4) run the ROS-2 interop pytest (isolated tree) =="
# Isolate the ros2/ tests so pytest does not collect sibling tests that import
# the `zerodds` python module (separate PyO3 binding, not needed here).
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp "$REPO"/crates/py/python/tests/ros2/*.py "$TMP/"
cd "$TMP"
# `launch_testing` and `launch_ros` register themselves as pytest plugins
# from the ROS install, and the tests here use neither. Jazzy's
# `launch_testing` declares the `path` argument of
# `pytest_pycollect_makemodule`, which pytest 9 removed: collection then stops
# with a PluginValidationError before a test runs (pytest 9.1.1 in the
# RoboStack env). `launch_ros` implements a hook `launch_testing` specifies,
# so it cannot load without it. Not loading either keeps the run independent
# of the two versions.
RMW_IMPLEMENTATION=rmw_zerodds_cpp \
  LD_LIBRARY_PATH="$ZERODDS_TARGET:$ROS_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  ROS_DOMAIN_ID="${ROS_DOMAIN_ID:-42}" \
  "$PYTHON" -m pytest . -v -p no:cacheprovider -p no:launch_testing -p no:launch_ros
