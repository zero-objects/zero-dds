#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# map-length.sh — the other reading of a delimited map's length, through
# the implementation that takes it.
#
#   tools/unspecified-reach/map-length.sh
#   tools/unspecified-reach/map-length.sh --out map-length.tsv
#
#   --work <dir>   where the probe is generated and compiled
#   --out <file>   the report, as TSV; default stdout
#   --require      OpenDDS absent fails the run
#
# One row per (case, encoding):
#
#   case  encoding  bytes  length
#
# Payload only; the four-byte encapsulation header is not written by
# `OpenDDS::DCPS::Serializer`.
#
# ## What it settles
#
# §7.4.1.1.4 defines a map by equivalence to `sequence<MapEntry_K_V>`,
# and rule (12) writes a sequence as DHEADER, then length, then the
# elements. Rule (15), written for `MAP_TYPE` itself, writes DHEADER and
# then the pairs with no length. Four bytes apart, and both are
# implemented: the byte gate already has Fast DDS taking the first, and
# this harness has OpenDDS taking the second.
#
# Neither stream is malformed and neither reader can detect the other's
# shape — a reader expecting rule (12) takes OpenDDS's first key for an
# entry count; a reader expecting rule (15) takes Fast DDS's count for a
# key. That is OMG issue DDSXTY14-72, and
# `docs/omg-xtypes-issues.md` §A1 is the comment.
#
# Under XCDR1 (rule 16) the two agree exactly, and a `map<long,long>` —
# rule (14), `PMAP_TYPE` — agrees in both versions. The rows for both
# are produced here so that the divergence is bounded by measurement
# rather than by assertion.
#
# Recorded as `crates/ast-layout/tests/fixtures/unspecified_constructs.md` §5.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../interop-vendors.sh
. "$HERE/../interop-vendors.sh"

WORK=""
OUT=""
REQUIRE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --work)    WORK="$2"; shift 2 ;;
        --out)     OUT="$2"; shift 2 ;;
        --require) REQUIRE=1; shift ;;
        *) echo "map-length.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

emit() { if [ -n "$OUT" ]; then printf '%s\n' "$*" >> "$OUT"; else printf '%s\n' "$*"; fi; }
[ -n "$OUT" ] && : > "$OUT"

if ! vendor_probe opendds; then
    vendor_skip opendds 6
    vendor_report
    [ $REQUIRE -eq 1 ] && exit 1
    exit 0
fi

WORK="${WORK:-$(mktemp -d "${TMPDIR:-/tmp}/unspec-map.XXXXXX")}"
mkdir -p "$WORK/shadow/dds"

cat > "$WORK/gen.idl" <<'I'
@final struct Cell { long a; long b; };
@final struct MapStruct { map<long, Cell> m; octet tail; };
@final struct MapLong   { map<long, long> m; octet tail; };
@final struct MapString { map<long, string> m; octet tail; };
I

# OpenDDS's headers are compiled with security on where the install has
# it, and this checkout's generated DdsSecurityCoreC.h carries no
# `DDS::Security` namespace, so `Discovery.h` will not compile. The
# config header is shadowed rather than edited: the vendor tree is never
# modified, and the setting affects nothing this program uses — the
# Serializer is the same code either way.
if [ -f "$OPENDDS_HOME/include/dds/OpenDDSConfig.h" ]; then
    sed 's/#define OPENDDS_CONFIG_SECURITY 1/#define OPENDDS_CONFIG_SECURITY 0/' \
        "$OPENDDS_HOME/include/dds/OpenDDSConfig.h" > "$WORK/shadow/dds/OpenDDSConfig.h"
elif [ -f "$DDS_ROOT/OpenDDSConfig.h" ]; then
    sed 's/#define OPENDDS_CONFIG_SECURITY 1/#define OPENDDS_CONFIG_SECURITY 0/' \
        "$DDS_ROOT/OpenDDSConfig.h" > "$WORK/shadow/dds/OpenDDSConfig.h"
fi

cat > "$WORK/ref.cpp" <<'CPP'
#include "genTypeSupportImpl.h"
#include <dds/DCPS/Serializer.h>
#include <cstdio>
using namespace OpenDDS::DCPS;

static void dump(const char *name, const char *encoding_name,
                 size_t length, const unsigned char *bytes) {
    printf("%s\t%s\t", name, encoding_name);
    for (size_t i = 0; i < length; i++) printf("%02x", bytes[i]);
    printf("\t%zu\n", length);
}

template <class T>
static void run(const char *name, const T &sample, Encoding::Kind kind,
                const char *encoding_name) {
    Encoding encoding(kind, ENDIAN_LITTLE);
    size_t size = serialized_size(encoding, sample);
    ACE_Message_Block block(size + 16);
    Serializer ser(&block, encoding);
    if (!(ser << sample)) { printf("%s\t%s\trefused\t0\n", name, encoding_name); return; }
    dump(name, encoding_name, block.length(),
         reinterpret_cast<const unsigned char *>(block.rd_ptr()));
}

int main() {
    MapStruct structured; Cell cell; cell.a(1); cell.b(2);
    structured.m()[7] = cell; structured.tail(0xAB);
    run("map_struct", structured, Encoding::KIND_XCDR1, "xcdr1");
    run("map_struct", structured, Encoding::KIND_XCDR2, "xcdr2");

    MapLong primitive; primitive.m()[1] = 2; primitive.tail(0xAB);
    run("map_long", primitive, Encoding::KIND_XCDR1, "xcdr1");
    run("map_long", primitive, Encoding::KIND_XCDR2, "xcdr2");

    MapString stringed; stringed.m()[7] = "abc"; stringed.tail(0xAB);
    run("map_string", stringed, Encoding::KIND_XCDR1, "xcdr1");
    run("map_string", stringed, Encoding::KIND_XCDR2, "xcdr2");
    return 0;
}
CPP

if ! ( cd "$WORK" && "$OPENDDS_HOME/bin/opendds_idl" -Lc++11 gen.idl > idl.log 2>&1 ); then
    echo "opendds: opendds_idl refused the IDL — see $WORK/idl.log" >&2
    tail -5 "$WORK/idl.log" >&2
    exit 1
fi

if ! ( cd "$WORK" && g++ -std=c++17 -o ref ref.cpp genTypeSupportImpl.cpp \
        -I. -Ishadow -I"$OPENDDS_HOME/include" \
        -L"$OPENDDS_HOME/lib" \
        -lOpenDDS_Dcps -lTAO_Valuetype -lTAO_AnyTypeCode -lTAO -lACE \
        -Wl,-rpath,"$OPENDDS_HOME/lib" > cc.log 2>&1 ); then
    echo "opendds: the probe did not compile — see $WORK/cc.log" >&2
    grep -m6 'error:' "$WORK/cc.log" >&2
    exit 1
fi

LD_LIBRARY_PATH="$OPENDDS_HOME/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
    "$WORK/ref" | while IFS= read -r line; do emit "$line"; done

vendor_report
exit 0
