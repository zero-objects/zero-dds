#!/usr/bin/env bash
# Build `rti_robot`, the RTI Connext client of the #28 interop gate.
#
# Like the Fast DDS cell's cmake step, this compiles OUR interop client
# against a pre-installed vendor SDK. It never builds Connext.
#
# The type support is generated from `../fastdds/robot.idl` — the same file
# the Fast DDS cell uses, not a copy — so the two vendor cells cannot drift
# into measuring two different `Robot`s.
#
#   NDDSHOME   the Connext install   (default: tools/interop-vendors.sh's)
#   RTI_TARGET the Connext target architecture
#
# Output: build/rti_robot
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
# shellcheck source=tools/interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

# The C++ libraries are named for the gcc that built them; the C target the
# byte probes use is a different string. `RTI_LIB` is the directory
# `tools/interop-vendors.sh` resolved, and the architecture is its basename.
ARCH="$(basename "$RTI_LIB")"
BUILD="$HERE/build"

[ -x "$NDDSHOME/bin/rtiddsgen" ] || {
    echo "rti/build.sh: no rtiddsgen at NDDSHOME=$NDDSHOME" >&2; exit 2; }
[ -f "$NDDSHOME/rti_license.dat" ] || {
    echo "rti/build.sh: NDDSHOME=$NDDSHOME has no rti_license.dat; rtiddsgen needs none but the Connext runtime does" >&2; exit 2; }

mkdir -p "$BUILD" || exit 2
cp -f "$HERE/rti_robot.cxx" "$BUILD/" || exit 2

( cd "$BUILD" || exit 2
  "$NDDSHOME/bin/rtiddsgen" -language C++11 -platform "$ARCH" -replace \
      -d . "$HERE/../fastdds/robot.idl" > rtiddsgen.log 2>&1 ) || {
    echo "rti/build.sh: rtiddsgen failed:" >&2
    tail -20 "$BUILD/rtiddsgen.log" >&2
    exit 1; }

( cd "$BUILD" || exit 2
  g++ -m64 -Wall -std=c++14 -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT -DRTI_STATIC \
      -I. -I"$NDDSHOME/include" -I"$NDDSHOME/include/ndds" \
      -I"$NDDSHOME/include/ndds/hpp" \
      rti_robot.cxx robot.cxx robotPlugin.cxx -o rti_robot \
      -L"$NDDSHOME/lib/$ARCH" -lnddscpp2z -lnddscz -lnddscorez \
      -ldl -lm -lpthread -lrt -no-pie -rdynamic > cc.log 2>&1 ) || {
    echo "rti/build.sh: g++ failed:" >&2
    grep -m5 'error:' "$BUILD/cc.log" >&2 || tail -20 "$BUILD/cc.log" >&2
    exit 1; }

echo "$BUILD/rti_robot"
