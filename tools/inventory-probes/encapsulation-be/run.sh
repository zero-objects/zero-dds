#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# encapsulation-be — B1's tenth identifier, `D_CDR2_BE = 0x0008`.
#
#   tools/inventory-probes/encapsulation-be/run.sh
#
# See `probe-opendds.cpp` for why OpenDDS is the implementation that can
# be asked and RTI and Fast DDS cannot: OpenDDS is the one whose
# serialiser takes the byte order as a run-time argument, so the
# big-endian half of every pair is reachable on an x86-64 host.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
# shellcheck source=../../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"
# shellcheck source=../build.sh
. "$ROOT/tools/inventory-probes/build.sh"

work="${INVENTORY_PROBE_WORK:-${TMPDIR:-/tmp}/inventory-probes}/encapsulation-be"
rm -rf "$work"; mkdir -p "$work"

vendor_probe opendds || { vendor_skip opendds 1; vendor_report; exit 0; }
d="$work/opendds"; mkdir -p "$d"
cp "$HERE/gen.idl" "$HERE/probe-opendds.cpp" "$d/"
build_cpp_opendds "$d" "$d/gen.idl" "$d/probe-opendds.cpp" || exit 1
printf 'type\tversion\tendian\tidentifier\tOpenDDS name\tbody\n'
( cd "$d" && ./probe )

vendor_report || true
