#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Builds the counterparty for `capture.sh`: eProsima Micro-XRCE-DDS Agent
# and Client, both at tag v3.0.1. Nothing in this script is ZeroDDS, and
# nothing in the tree it produces is — that is the point of it.
#
#   XRCE_ROOT=~/xrce-foreign tools/wire-capture/xrce-sessions/build-foreign.sh
#
# The two source trees have to be checked out under $XRCE_ROOT first:
#
#   git clone -b v3.0.1 https://github.com/eProsima/Micro-XRCE-DDS-Agent  "$XRCE_ROOT/agent"
#   git clone -b v3.0.1 https://github.com/eProsima/Micro-XRCE-DDS-Client "$XRCE_ROOT/client"
#
# The commits the recorded session was taken from are in README.md.
#
# Both builds are CMake superbuilds: the agent fetches Fast DDS and Fast
# CDR, the client fetches Micro CDR. That is deliberate — the versions
# are the ones the reference implementation pins for that tag, and a
# system Fast DDS would make the capture a measurement of this host's
# package set rather than of the reference implementation.

set -euo pipefail

ROOT="${XRCE_ROOT:-$HOME/xrce-foreign}"
INSTALL="$ROOT/install"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"

for half in agent client; do
    if [ ! -f "$ROOT/$half/CMakeLists.txt" ]; then
        echo "NOT PRESENT: $ROOT/$half — clone it first, see the header" >&2
        exit 2
    fi
done

mkdir -p "$INSTALL"

echo "=== agent ==="
cmake -S "$ROOT/agent" -B "$ROOT/agent/build" \
    -DCMAKE_INSTALL_PREFIX="$INSTALL" \
    -DCMAKE_BUILD_TYPE=Release \
    -DUAGENT_BUILD_TESTS=OFF \
    -DUAGENT_BUILD_EXECUTABLE=ON \
    -DUAGENT_SUPERBUILD=ON
cmake --build "$ROOT/agent/build" -j "$JOBS" --target install

echo "=== client ==="
cmake -S "$ROOT/client" -B "$ROOT/client/build" \
    -DCMAKE_INSTALL_PREFIX="$INSTALL" \
    -DCMAKE_BUILD_TYPE=Release \
    -DUCLIENT_BUILD_EXAMPLES=ON \
    -DUCLIENT_SUPERBUILD=ON
cmake --build "$ROOT/client/build" -j "$JOBS" --target install

echo "=== what came out ==="
find "$ROOT" \
    -name MicroXRCEAgent \
    -o -name 'PublishHelloWorld*' \
    -o -name 'SubscribeHelloWorld*' \
    | sort
