#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
idlc=${1:-"$repo/target/debug/zerodds-idlc"}
generated=$(mktemp -d)
cd "$repo"
schema=crates/java-omgdds/idl/typelookup/ddsi_xt_typelookup.idl
destination="$repo/crates/java-omgdds/java/src/main/java/org/zerodds/typelookup/generated"
# One codec in two encodings: ZdCodec is XCDR2 (CDR2_LE, the default of the
# TypeLookup endpoints, DDS-XTypes 1.4 Beta 1 section 7.6.3.3.3), its nested
# ZdCodec.Xcdr1 XCDR1 (CDR_LE, the setting xcdr1, DDSI-RTPS 2.5 section 10.4),
# which the generator writes beside the XCDR2 codecs for the RPC request and
# reply types of the IDL.
"$idlc" generate --lang java --java-mapping omg --encoding xcdr2 --out "$generated/x2" "$schema"
mkdir -p "$destination"
# Mechanical Java package placement only. Protocol source/codecs are generated
# from the unmodified vendor IDL, never handwritten or byte-patched. The
# service support (ZdServices, ZdTypeSupports) is not used here.
for file in ZdTypes.java ZdCodec.java; do
    awk 'BEGIN { print "package org.zerodds.typelookup.generated;"; print "" } { print }' "$generated/x2/$file" > "$destination/$file"
done
printf 'Generated TypeLookup sources from %s in %s\n' "$schema" "$generated"
