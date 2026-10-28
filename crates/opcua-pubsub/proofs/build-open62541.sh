#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Builds the open62541 the two OPC-UA proofs run against.
#
#   O62_SRC=~/open62541 crates/opcua-pubsub/proofs/build-open62541.sh
#
# It builds the source tree **twice**, into two directories, and that is
# not redundancy — the two proofs need opposite settings of one option:
#
#   $O62_BUILD        UA_ENABLE_PUBSUB_INFORMATIONMODEL=ON
#                    for crates/opcua-server/proofs/open62541.sh, which
#                    browses a server's address space and wants the full
#                    one.
#
#   $O62_BUILD_NOPIM  UA_ENABLE_PUBSUB_INFORMATIONMODEL=OFF
#                    for crates/opcua-pubsub/proofs/open62541_uadp.sh
#                    direction B. `tutorial_pubsub_subscribe.c` hardcodes
#                    its target variables at ns=1;i=50000.., which the
#                    PubSub information model already occupies; with the
#                    model on, the example's own addVariableNode calls
#                    fail and every received field is dropped with
#                    Bad_NodeIdUnknown. That is a property of the foreign
#                    example, not of the wire — and it looks exactly like
#                    a decode failure on our side.
#
# The version this was written against is open62541 1.5.4, master at
# `5568261` (2026-06-23). The source tree is not vendored here: it is
# upstream's, and a build of it is 100 MB per configuration.
#
# The proofs find the result through O62_EXAMPLES / O62_LIB / O62_SRC /
# O62_GEN; this script prints the four assignments it produced.

set -euo pipefail

SRC="${O62_SRC:-/root/open62541}"
# The two defaults are the directory names on the measurement host, which
# is also what the two proofs default to. Nothing depends on the names —
# what depends on anything is that the -nopim one is the one built with
# UA_ENABLE_PUBSUB_INFORMATIONMODEL=OFF.
BUILD="${O62_BUILD:-/root/o62-ex}"
BUILD_NOPIM="${O62_BUILD_NOPIM:-/root/o62-ex2}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"

if [ ! -f "$SRC/CMakeLists.txt" ]; then
    echo "NOT PRESENT: $SRC — clone https://github.com/open62541/open62541 first" >&2
    exit 2
fi

build() {
    local dir="$1" pim="$2"
    echo "=== $dir (UA_ENABLE_PUBSUB_INFORMATIONMODEL=$pim) ==="
    cmake -S "$SRC" -B "$dir" \
        -DCMAKE_BUILD_TYPE=Release \
        -DUA_BUILD_EXAMPLES=ON \
        -DUA_ENABLE_PUBSUB=ON \
        -DUA_ENABLE_PUBSUB_INFORMATIONMODEL="$pim" \
        -DUA_ENABLE_SUBSCRIPTIONS=ON \
        -DUA_ENABLE_METHODCALLS=ON \
        -DUA_ENABLE_NODEMANAGEMENT=ON \
        -DUA_ENABLE_DIAGNOSTICS=ON \
        -DUA_ENABLE_JSON_ENCODING=ON \
        -DUA_ENABLE_XML_ENCODING=ON \
        -DUA_ENABLE_DATATYPES_ALL=ON \
        -DUA_ENABLE_TYPEDESCRIPTION=ON \
        -DUA_ENABLE_STATUSCODE_DESCRIPTIONS=ON \
        -DUA_NAMESPACE_ZERO=REDUCED
    cmake --build "$dir" -j "$JOBS"
}

build "$BUILD"       ON
build "$BUILD_NOPIM" OFF

cat <<EOF

=== what to export ===

# crates/opcua-server/proofs/open62541.sh
export O62_EXAMPLES=$BUILD/bin/examples

# crates/opcua-pubsub/proofs/open62541_uadp.sh
export O62_EXAMPLES=$BUILD_NOPIM/bin/examples
export O62_LIB=$BUILD_NOPIM/bin
export O62_GEN=$BUILD_NOPIM/src_generated
export O62_SRC=$SRC
EOF
