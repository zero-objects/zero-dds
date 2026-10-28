#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# run.sh — does an RTI reader keep one instance for a key that an RTI
# writer and a ZeroDDS writer both write under XCDR1?
#
# The key holder is `@key short a; @key long long b;`: PLAIN_CDR2, which
# XTypes 1.3 §7.6.8 step 4 prescribes for the KeyHash, puts `b` at 4,
# XCDR1 at 8. `pair_live` (RTI) and `keyhash_pair` (ZeroDDS) each write
# {a = 1, b = 2} and read every sample on the topic; each prints the
# instance a sample lands in. One instance per reader is the pass.
#
#   tools/keyhash-probe/rti-live/run.sh [domain]
#
# Needs RTI Connext 7.7.0 (`NDDSHOME`, `RTI_LICENSE_FILE`, see
# tools/interop-vendors.sh). Measured on the Linux test host:
# RTI 20 + 15 samples in instance 00010000000000000000000200000000, ZeroDDS
# 15 + 18 samples in one instance.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
# shellcheck source=../../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"
RTI_LICENSE_FILE="${RTI_LICENSE_FILE:-$NDDSHOME/rti_license.dat}"
export RTI_LICENSE_FILE
domain="${1:-46}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cp "$HERE/pair.idl" "$HERE/pair_live.c" "$work/"
( cd "$work"
  "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" -replace pair.idl > gen.log 2>&1
  gcc -O0 -g -o pair_live pair_live.c pair.c pairPlugin.c pairSupport.c \
      -I. -I"$NDDSHOME/include" -I"$NDDSHOME/include/ndds" -L"$RTI_LIB" \
      -lnddsc -lnddscore -ldl -lm -lpthread -lrt -Wl,-rpath,"$RTI_LIB" \
      -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT )
( cd "$ROOT" && cargo build -q -p zerodds-dcps --example keyhash_pair )
zd="${CARGO_TARGET_DIR:-$ROOT/target}/debug/examples/keyhash_pair"

timeout 20 "$work/pair_live" "$domain" 10 1000 > "$work/rti.log" 2>&1 &
rti=$!
sleep 2
timeout 20 "$zd" "$domain" 9 2000 1 > "$work/zd.log" 2>&1
wait "$rti" || true

# v = 1000.. comes from RTI's writer, v = 2000.. from ZeroDDS's.
tally() {
    awk -v who="$1" '$1 == who {
        split($2, v, "="); src = (v[2] < 2000) ? "rti-writer" : "zerodds-writer"
        print src, $NF }' "$2" | sort | uniq -c
}
echo "RTI reader:";     tally rti-reader "$work/rti.log"
echo "ZeroDDS reader:"; tally zerodds-reader "$work/zd.log"
