#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# bitset-bytes.sh — a bitset in a collection and as a key member,
# through the one implementation whose front end parses the keyword.
#
#   tools/unspecified-reach/bitset-bytes.sh
#   tools/unspecified-reach/bitset-bytes.sh --shapes seq_bitset,key_bitset
#
#   --shapes <list|all>   default all
#   --work <dir>          where the probes are generated and compiled
#   --out <file>          the report, as TSV; default stdout
#   --require             Fast DDS absent fails the run
#
# One row per (shape, form):
#
#   shape  form  bytes
#
# `form` is `payload-xcdr1`, `payload-xcdr2`, `keyholder-xcdr1-be`,
# `keyholder-xcdr2-be` or `instance-handle`. Payload rows exclude the
# four-byte encapsulation header. The key holder is big endian because
# §7.6.8 Step 4 says it is.
#
# Rows P1, P2, P3 and P5 of `docs/wire-feature-inventory.md` rest on
# these, and `front-ends.sh` beside this script is why there is only one
# vendor here: Cyclone, RTI, OpenDDS, TAO and omniORB all reject
# `bitset` with a syntax error. That is a measurement about the
# construct and not a gap in this harness.
#
# The recorded results are
# `crates/ast-layout/tests/fixtures/unspecified_constructs.md` §1, and
# `cargo test -p zerodds-wire-probe --test spec_constructs` is the side
# of it that needs no vendor.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../interop-vendors.sh
. "$HERE/../interop-vendors.sh"

SHAPES=all
WORK=""
OUT=""
REQUIRE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --shapes)  SHAPES="$2"; shift 2 ;;
        --work)    WORK="$2"; shift 2 ;;
        --out)     OUT="$2"; shift 2 ;;
        --require) REQUIRE=1; shift ;;
        *) echo "bitset-bytes.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

ALL_SHAPES="bitset_scalar seq_bitset array_bitset seq_bitset_wide key_bitset key_bitset_long"
[ "$SHAPES" = all ] && SHAPES="$ALL_SHAPES"
SHAPE_LIST="${SHAPES//,/ }"
shape_count=$(printf '%s\n' $SHAPE_LIST | grep -c .)

emit() { if [ -n "$OUT" ]; then printf '%s\n' "$*" >> "$OUT"; else printf '%s\n' "$*"; fi; }
[ -n "$OUT" ] && : > "$OUT"

if ! vendor_probe fastdds; then
    vendor_skip fastdds $((shape_count * 5))
    vendor_report
    [ $REQUIRE -eq 1 ] && exit 1
    exit 0
fi

WORK="${WORK:-$(mktemp -d "${TMPDIR:-/tmp}/unspec-bitset.XXXXXX")}"
mkdir -p "$WORK"

# idl_for <shape>; fill_for <shape> — kept as functions rather than case
# directories because every one of them is three lines and the value in
# each is chosen to move if a width, an offset or a bit order is wrong.
idl_for() {
    local bs='bitset Packed { bitfield<3> low; bitfield<5> high; };'
    local wide='bitset Wide { bitfield<12> a; bitfield<12> b; };'
    case "$1" in
        bitset_scalar)   printf '%s\n@final struct Sample { Packed v; octet tail; };\n' "$bs" ;;
        seq_bitset)      printf '%s\n@final struct Sample { sequence<Packed> many; long tail; };\n' "$bs" ;;
        array_bitset)    printf '%s\n@final struct Sample { Packed items[2]; octet tail; };\n' "$bs" ;;
        seq_bitset_wide) printf '%s\n@final struct Sample { sequence<Wide> many; octet tail; };\n' "$wide" ;;
        key_bitset)      printf '%s\n@final struct Sample { @key Packed k; octet tail; };\n' "$bs" ;;
        key_bitset_long) printf '%s\n@final struct Sample { @key Wide k; @key string<40> s; octet tail; };\n' "$wide" ;;
        *) return 1 ;;
    esac
}
fill_for() {
    case "$1" in
        bitset_scalar)   echo '  sample.v().low = 5; sample.v().high = 17; sample.tail(0xAB);' ;;
        seq_bitset)      echo '  Packed a; a.low=5; a.high=17; Packed b; b.low=0; b.high=31; sample.many({a,b}); sample.tail(-1);' ;;
        array_bitset)    echo '  sample.items()[0].low=5; sample.items()[0].high=17; sample.items()[1].low=7; sample.items()[1].high=0; sample.tail(0xAB);' ;;
        seq_bitset_wide) echo '  Wide a; a.a=0x111; a.b=0x222; Wide b; b.a=1; b.b=2; sample.many({a,b}); sample.tail(0xAB);' ;;
        key_bitset)      echo '  sample.k().low = 5; sample.k().high = 17; sample.tail(0xAB);' ;;
        key_bitset_long) echo '  sample.k().a=0x111; sample.k().b=0x222; sample.s("abc"); sample.tail(0xAB);' ;;
        *) return 1 ;;
    esac
}

for shape in $SHAPE_LIST; do
    dir="$WORK/$shape"; mkdir -p "$dir"
    idl_for "$shape" > "$dir/gen.idl" || { echo "bitset-bytes.sh: no shape $shape" >&2; exit 2; }
    {
        cat <<'CPP'
#include <cstdio>
#include <fastcdr/Cdr.h>
#include <fastcdr/FastBuffer.h>
#include "gen.hpp"
#include "genCdrAux.hpp"
#include "genPubSubTypes.hpp"
using eprosima::fastcdr::Cdr;
using eprosima::fastcdr::CdrVersion;
using eprosima::fastcdr::FastBuffer;

// `key` selects §7.6.8's key holder, which is big endian by the clause
// and not by choice, and `serialize_key` rather than `operator<<`.
static void dump(const char *form, CdrVersion version, const Sample &sample, bool key) {
    char raw[8192];
    FastBuffer buffer(raw, sizeof(raw));
    Cdr ser(buffer, key ? Cdr::BIG_ENDIANNESS : Cdr::LITTLE_ENDIANNESS, version);
    ser.serialize_encapsulation();
    if (key) eprosima::fastcdr::serialize_key(ser, sample); else ser << sample;
    const size_t length = ser.get_serialized_data_length();
    printf("%s\t", form);
    for (size_t i = 4; i < length; i++)
        printf("%02x", static_cast<unsigned char>(raw[i]));
    printf("\n");
}

int main() {
    Sample sample;
CPP
        fill_for "$shape"
        cat <<'CPP'
    dump("payload-xcdr1",      CdrVersion::XCDRv1, sample, false);
    dump("payload-xcdr2",      CdrVersion::XCDRv2, sample, false);
    dump("keyholder-xcdr1-be", CdrVersion::XCDRv1, sample, true);
    dump("keyholder-xcdr2-be", CdrVersion::XCDRv2, sample, true);
    SamplePubSubType support;
    eprosima::fastdds::rtps::InstanceHandle_t handle;
    if (support.compute_key(&sample, handle, false)) {
        printf("instance-handle\t");
        for (int i = 0; i < 16; i++) printf("%02x", handle.value[i]);
        printf("\n");
    } else {
        // A keyless type has no KeyHash (N8), which is a result and not
        // a failure — it is reported rather than left out of the report.
        printf("instance-handle\tnone\n");
    }
    return 0;
}
CPP
    } > "$dir/ref.cpp"

    # `fastddsgen` is a JVM tool and two of them started a second apart
    # collide on /tmp/hsperfdata_<user>, which fails the generation for a
    # reason that has nothing to do with the IDL. Retried once, and the
    # second failure is reported with its diagnostic rather than as a
    # bare status — a row that said `uncompiled` and nothing else would
    # be a silent skip wearing a status code.
    ( cd "$dir" || exit 3
      if ! "$FASTDDSGEN" -replace gen.idl > gen.log 2>&1; then
          sleep 2
          if ! "$FASTDDSGEN" -replace gen.idl > gen.log 2>&1; then
              printf 'ungenerated\t%s\n' \
                  "$(grep -im1 'error\|exception\|not supported' gen.log \
                     | sed 's/\x1b\[[0-9;]*m//g' | tr -d '\r' | cut -c1-160)"
              exit 0
          fi
      fi
      if ! g++ -std=c++17 -o ref ref.cpp genPubSubTypes.cxx genTypeObjectSupport.cxx \
              -I. -I"$FASTDDS_HOME/include" -L"$FASTDDS_HOME/lib" -lfastcdr -lfastdds \
              -Wl,-rpath,"$FASTDDS_HOME/lib" > cc.log 2>&1; then
          printf 'uncompiled\t%s\n' \
              "$(grep -m1 'error:' cc.log | tr -d '\r' | cut -c1-160)"
          exit 0
      fi
      ./ref ) | while IFS= read -r line; do
        emit "$(printf '%s\t%s' "$shape" "$line")"
    done
done

vendor_report
exit 0
