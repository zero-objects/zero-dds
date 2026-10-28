#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
idlc=${1:-"$repo/target/debug/zerodds-idlc"}
generated=$(mktemp -d)
cd "$repo"
schema=crates/java-omgdds/idl/typeobject/dds_xtypes_11_typeobject.idl
destination="$repo/crates/java-omgdds/java/src/main/java/org/zerodds/typeobject/generated"
"$idlc" generate --lang java --java-mapping omg --encoding xcdr1 --out "$generated" "$schema"
mkdir -p "$destination"
# Mechanical Java package placement only. Protocol source/codecs are generated
# from the unmodified vendor IDL, never handwritten or byte-patched.
for file in ZdTypes.java ZdCodec.java; do
    awk 'BEGIN { print "package org.zerodds.typeobject.generated;"; print "" } { print }' "$generated/$file" > "$destination/$file"
done
printf 'Generated TypeLookup sources from %s in %s\n' "$schema" "$generated"
