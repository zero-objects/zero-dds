#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# fixed-orbs.sh — `fixed<P,S>` through omniORB and JacORB.
#
#   tools/wire-oracle/fixed-orbs.sh            both, if both are there
#   tools/wire-oracle/fixed-orbs.sh omniorb
#
# No DDS specification has a fixed-point type, so no DDS generator can
# be asked about one and the corpus has no case that carries it. This is
# the shape the question has to take instead: one struct holding nine
# `fixed` typedefs that vary precision, scale, sign and size — including
# `fixed<31,0>`, the widest CORBA allows — with an `octet` in front and
# another behind so the padding on both sides is visible.
#
# It is one whole struct through each ORB's generated marshaller and no
# case directory, because every member after a mis-marshalled `fixed`
# would be unreadable and a per-member case could not tell which member
# went wrong.
#
# `omni/r.idl` and `omni/b.idl` beside it are the two refusals: a
# `fixed<31,31>` that omniidl rejects, and the `bitset` no CORBA front
# end parses. They are kept because a refusal is a measurement of a
# front end and re-deriving it needs the file that provoked it.
#
# ## What this replaces
#
# `/root/wire-oracle/omni/` and `/root/wire-oracle/jacorb/`. The
# `*Helper.java`/`*Holder.java` files that were beside `Ref.java` on the
# host are `org.jacorb.idl.parser` output and are regenerated here.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

work="${WIRE_ORACLE_WORK:-${TMPDIR:-/tmp}/wire-oracle-fixed}"
want="${*:-omniorb jacorb}"

run_omniorb() {
    local w="$work/omni"; rm -rf "$w"; mkdir -p "$w"
    cp "$HERE/omni/gen.idl" "$HERE/omni/ref.cpp" "$w/"
    ( cd "$w" \
      && "$OMNIIDL" -bcxx gen.idl > omniidl.log 2>&1 \
      && g++ -std=c++17 -o ref ref.cpp genSK.cc -I. -I"$OMNIORB_INCLUDE/.." \
             -D__x86__ -D__linux__ -D__OSVERSION__=2 \
             -lomniORB4 -lomnithread -lomniDynamic4 > cc.log 2>&1 ) \
      || { echo "fixed-orbs: the omniORB probe did not build" >&2
           grep -m4 -i error "$w"/*.log >&2; return 1; }
    "$w/ref"
}

run_jacorb() {
    local w="$work/jacorb"; rm -rf "$w"; mkdir -p "$w"
    cp "$HERE/jacorb/gen.idl" "$HERE/jacorb/Ref.java" "$w/"
    local cp_jars; cp_jars="$JACORB_HOME/lib/*:$w"
    ( cd "$w" \
      && java -cp "$JACORB_HOME/lib/*" org.jacorb.idl.parser -d . gen.idl > idl.log 2>&1 \
      && javac -cp "$cp_jars" -d . ./*.java > javac.log 2>&1 ) \
      || { echo "fixed-orbs: the JacORB probe did not build" >&2
           grep -m4 -i error "$w"/*.log >&2; return 1; }
    java -cp "$cp_jars" Ref
}

for orb in $want; do
    if vendor_probe "$orb"; then
        "run_$orb" || true
    else
        # One struct, one byte order each for JacORB and two for omniORB.
        vendor_skip "$orb" "$( [ "$orb" = omniorb ] && echo 2 || echo 1 )"
    fi
done

vendor_report || true
