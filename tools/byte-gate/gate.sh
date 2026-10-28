#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# byte-gate — one IDL type, one fixed sample, and the bytes each foreign
# implementation's own serialiser produces for it.
#
#   tools/byte-gate/gate.sh --cases point
#   tools/byte-gate/gate.sh --vendors cyclone,fastdds --endians le,be
#   tools/byte-gate/gate.sh --out regenerated.tsv
#
#   --cases <list|all>    default all; names are directories under cases/
#   --vendors <list>      default cyclone,rti,fastdds,opendds
#   --endians <list>      default le,be
#   --corpus <dir>        default cases/ beside this script
#   --work <dir>          where probes are generated and compiled
#   --out <file>          the report, as TSV; default stdout
#   --require             a vendor that is not installed fails the run
#
# One row per (case, vendor, encoding, endian):
#
#   case  vendor  encoding  endian  status  length  header  bytes
#
# `status` is `wrote`, `refused` (the vendor's serialiser said no),
# `ungenerated` (its IDL front end would not parse the type),
# `uncompiled` (the generated code would not build) or `unfilled` (this
# harness has no sample in that vendor's language mapping). The four
# failures are kept apart because they are findings of different weight:
# a front end that refuses a keyword is a statement about the language
# it accepts, a serialiser that refuses a sample is a statement about
# the wire, and `unfilled` is a statement about this repository.
#
# `length` and `bytes` exclude the four-byte encapsulation header.
# `header` carries it for the two vendors that write one and is empty
# for the two that do not, which is itself a measurement — see the
# template comments.
#
# ## What this replaces
#
# `/root/byte-gate/gate/gate.sh` and `/root/byte-gate/gate-be/run.sh` on
# the Linux test host, which is where every byte in
# `crates/ast-layout/tests/fixtures/vendor_bytes.md` was measured before
# this file existed. The cases are the same directories; the templates
# are the same code with their output turned into rows.
#
# ## Toolchains
#
# `tools/interop-vendors.sh` says where each one is expected and what to
# set when it is somewhere else. A vendor that is not installed is named
# on stderr with the number of rows it did not produce, and the run
# continues — never a silent skip, and never a row quietly missing from
# the report.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

corpus="$HERE/cases"
tpl="$HERE/tpl"
work="${BYTE_GATE_WORK:-${TMPDIR:-/tmp}/byte-gate-work}"
vendors="cyclone,rti,fastdds,opendds"
endians="le,be"
cases="all"
out=""
require=0

while [ $# -gt 0 ]; do
    case "$1" in
        --cases) cases="$2"; shift 2 ;;
        --vendors) vendors="$2"; shift 2 ;;
        --endians) endians="$2"; shift 2 ;;
        --corpus) corpus="$2"; shift 2 ;;
        --work) work="$2"; shift 2 ;;
        --out) out="$2"; shift 2 ;;
        --require) require=1; shift ;;
        --help|-h) sed -n '5,45p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "byte-gate: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done

[ -d "$corpus" ] || { echo "byte-gate: no case directory at $corpus" >&2; exit 2; }
if [ "$cases" = all ]; then
    cases="$(cd "$corpus" && for d in */; do [ -f "$d/type" ] && basename "$d"; done | sort | paste -sd, -)"
fi

case_count=$(printf '%s\n' "${cases//,/$'\n'}" | grep -c .)

# A vendor that cannot run is reported once, with the number of rows the
# run therefore does not carry: cases x encodings x the byte orders that
# vendor can produce. RTI's little-endian path is native-only, so its
# per-endian arithmetic is the same as everyone else's here.
present=""
for vendor in ${vendors//,/ }; do
    endian_count=$(printf '%s\n' "${endians//,/$'\n'}" | grep -c .)
    if vendor_probe "$vendor"; then
        present="$present $vendor"
    else
        vendor_skip "$vendor" $((case_count * 2 * endian_count))
    fi
done

mkdir -p "$work"

# A row for something that never reached the serialiser. `length` and
# `header` are empty and the reason takes the `bytes` column, so a
# failure and a measurement have the same eight columns and one parser.
fail_row() {
    # The reason is the vendor's own sentence, and a front end names the
    # file it was handed: `idlc` names it absolutely, so the run's work
    # directory ended up inside the row and then inside the record. 72
    # recorded rows carried `/tmp/byte-gate-work/<case>/<vendor>-<endian>/`
    # and compared equal only on a run that happened to use the default
    # `--work` — every run with `BYTE_GATE_WORK` set somewhere else read
    # as 72 changed rows. The directory is a run parameter and not a
    # measurement, so it comes out here, at the one place every reason
    # passes through. `w` is `one`'s local, visible because bash scopes
    # `local` dynamically.
    local reason="$6"
    if [ -n "${w:-}" ]; then reason="${reason//$w\//}"; fi
    printf '%s\t%s\t%s\t%s\t%s\t\t\t%s\n' "$1" "$2" "$3" "$4" "$5" "$reason"
}

# The same for both encoding versions, which is what a front end or a
# compiler failing costs: neither version was ever asked.
fail_both() {
    fail_row "$1" "$2" xcdr1 "$3" "$4" "$5"
    fail_row "$1" "$2" xcdr2 "$3" "$4" "$5"
}

# Build one probe for one (case, vendor, endian) and print its rows.
# Each probe emits the two encoding versions itself, because generating
# and compiling is the expensive part and the encoding version is a
# runtime argument everywhere except RTI's big-endian path.
one() {
    local case_name="$1" vendor="$2" endian="$3"
    local dir="$corpus/$case_name"
    local type_name; type_name="$(cat "$dir/type")"
    local cpp_type; cpp_type="$( [ -f "$dir/type.cpp" ] && cat "$dir/type.cpp" || echo "$type_name" )"
    local w="$work/$case_name/$vendor-$endian"
    rm -rf "$w"; mkdir -p "$w"
    cp "$dir/type.idl" "$w/gen.idl"

    case "$vendor" in
      cyclone)
        cp "$dir/fill.inc" "$w/fill.inc"
        local stream=LE; [ "$endian" = be ] && stream=BE
        sed -e "s/@TYPE@/$type_name/g" -e "s/@STREAM@/$stream/g" \
            -e "s/@ENDIAN@/$endian/g" "$tpl/cyclone.c" > "$w/ref.c"
        ( cd "$w"
          "$CYCLONE_HOME/bin/idlc" gen.idl > idlc.log 2>&1 ) || {
            fail_both "$case_name" cyclone "$endian" ungenerated \
                "$(grep -m1 . "$w/idlc.log" | tr '\t' ' ')"
            return 0; }
        ( cd "$w"
          gcc -o ref ref.c gen.c -I. -I"$CYCLONE_HOME/include" -L"$CYCLONE_HOME/lib" \
              -lddsc -Wl,-rpath,"$CYCLONE_HOME/lib" > cc.log 2>&1 ) || {
            fail_both "$case_name" cyclone "$endian" uncompiled \
                "$(grep -m1 'error:' "$w/cc.log" | tr '\t' ' ')"
            return 0; }
        "$w/ref" | sed "s/^/$case_name\tcyclone\t/"
        ;;

      fastdds)
        # A case with no C++ fill is a row and not a silence, for the same
        # reason as OpenDDS below: absence read as "Fast DDS had nothing
        # to say about this case" would be wrong — what is missing is
        # this harness's sample in Fast DDS's language mapping.
        if [ ! -f "$dir/fill.cpp.inc" ]; then
            fail_both "$case_name" fastdds "$endian" unfilled \
                "no fill.cpp.inc; the C fill assigns to members Fast DDS's C++ mapping reaches through setters"
            return 0
        fi
        cp "$dir/fill.cpp.inc" "$w/fill.inc"
        local endianness=LITTLE_ENDIANNESS; [ "$endian" = be ] && endianness=BIG_ENDIANNESS
        sed -e "s/@TYPE@/$cpp_type/g" -e "s/@ENDIANNESS@/$endianness/g" \
            -e "s/@ENDIAN@/$endian/g" "$tpl/fastdds.cpp" > "$w/ref.cpp"
        ( cd "$w"
          "$FASTDDSGEN" -replace gen.idl > gen.log 2>&1 ) || {
            fail_both "$case_name" fastdds "$endian" ungenerated \
                "$(sed 's/\x1b\[[0-9;]*m//g' "$w/gen.log" | tail -3 | tr '\n\t' '  ')"
            return 0; }
        ( cd "$w"
          g++ -std=c++17 -o ref ref.cpp genPubSubTypes.cxx genTypeObjectSupport.cxx \
              -I. -I"$FASTDDS_HOME/include" -L"$FASTDDS_HOME/lib" -lfastcdr -lfastdds \
              -Wl,-rpath,"$FASTDDS_HOME/lib" > cc.log 2>&1 ) || {
            fail_both "$case_name" fastdds "$endian" uncompiled \
                "$(grep -m1 'error:' "$w/cc.log" | tr '\t' ' ')"
            return 0; }
        "$w/ref" | sed "s/^/$case_name\tfastdds\t/"
        ;;

      opendds)
        # OpenDDS's C++ mapping assigns to members where Fast DDS's calls
        # setters, so a case needs its own fill and cannot borrow the
        # C++ one. Saying so is a row and not a silence: without it a
        # reader would read the absence as OpenDDS having nothing to say
        # about the case, when what is missing is this harness's fill.
        if [ -f "$dir/fill.odds.inc" ]; then cp "$dir/fill.odds.inc" "$w/fill.inc"
        else
            fail_both "$case_name" opendds "$endian" unfilled \
                "no fill.odds.inc; the Fast DDS fill calls setters OpenDDS's C++ mapping does not have"
            return 0
        fi
        sed -e "s/@TYPE@/$cpp_type/g" "$tpl/opendds.cpp" > "$w/ref.cpp"
        ( cd "$w"
          "$TAO_IDL" -I"$OPENDDS_HOME/include" -I"$TAO_ROOT" -Sa -St --idl-version 4 gen.idl > tao.log 2>&1 \
            && "$OPENDDS_HOME/bin/opendds_idl" -I"$DDS_ROOT" --idl-version 4 gen.idl > idl.log 2>&1 ) || {
            fail_both "$case_name" opendds "$endian" ungenerated \
                "$(grep -m1 -i error "$w/tao.log" "$w/idl.log" 2>/dev/null | tr '\t' ' ')"
            return 0; }
        ( cd "$w"
          g++ -std=c++17 -o ref ref.cpp genC.cpp genTypeSupportImpl.cpp -I. \
              -I"$OPENDDS_HOME/include" -L"$OPENDDS_HOME/lib" \
              -lOpenDDS_Dcps -lTAO_AnyTypeCode -lTAO_Valuetype -lTAO -lACE \
              -Wl,-rpath,"$OPENDDS_HOME/lib" > cc.log 2>&1 ) || {
            fail_both "$case_name" opendds "$endian" uncompiled \
                "$(grep -m1 'error:' "$w/cc.log" | tr '\t' ' ')"
            return 0; }
        # One binary produces all four combinations; keep the requested ones.
        LD_LIBRARY_PATH="$OPENDDS_HOME/lib" "$w/ref" \
            | awk -v e="$endian" -F'\t' '$2 == e' \
            | sed "s/^/$case_name\topendds\t/"
        ;;

      rti)
        cp "${dir}/$( [ -f "$dir/fill.rti.inc" ] && echo fill.rti.inc || echo fill.inc )" "$w/fill.inc"
        ( cd "$w"
          "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" -replace gen.idl > gen.log 2>&1 ) || {
            fail_both "$case_name" rti "$endian" ungenerated \
                "$(grep -m1 -i error "$w/gen.log" | tr '\t' ' ')"
            return 0; }
        if [ "$endian" = le ]; then
            sed -e "s/@TYPE@/$type_name/g" "$tpl/rti.c" > "$w/ref.c"
            ( cd "$w"
              gcc -o ref ref.c gen.c genPlugin.c genSupport.c -I. -I"$NDDSHOME/include" \
                  -I"$NDDSHOME/include/ndds" -L"$RTI_LIB" -lnddsc -lnddscore -ldl -lm -lpthread -lrt \
                  -Wl,-rpath,"$RTI_LIB" -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT > cc.log 2>&1 ) || {
                fail_both "$case_name" rti le uncompiled \
                    "$(grep -m1 'error:' "$w/cc.log" | tr '\t' ' ')"
                return 0; }
            "$w/ref" | sed "s/^/$case_name\trti\t/"
        else
            # `serialize_to_cdr_buffer_ex` is native-endian by definition,
            # so the big-endian rows go through the wrapper's own body —
            # `tools/wireprobe/rti-template.c`, shared rather than copied
            # so the two cannot drift — with the encapsulation id's low
            # bit cleared.
            #
            # The byte order and the representation are that probe's two
            # arguments and not `sed` markers: it stopped substituting
            # `@BIG_ENDIAN@` and `@REPRESENTATION@` when they moved into
            # `argv`, and this call did not follow. A probe called with no
            # argument prints its usage and exits 2, which arrived here as
            # `refused` — all 94 big-endian rows, on every case, reading
            # like a statement about RTI when it was a statement about
            # this file. One build now answers both encoding versions,
            # called the way `tools/wireprobe/wireprobe-rti` calls it.
            sed -e "s/@TYPE@/$type_name/g" \
                "$ROOT/tools/wireprobe/rti-template.c" > "$w/ref-be.c"
            ( cd "$w"
              gcc -o ref-be ref-be.c gen.c genPlugin.c genSupport.c -I. \
                  -I"$NDDSHOME/include" -I"$NDDSHOME/include/ndds" -L"$RTI_LIB" \
                  -lnddsc -lnddscore -ldl -lm -lpthread -lrt -Wl,-rpath,"$RTI_LIB" \
                  -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT > cc-be.log 2>&1 ) || {
                fail_both "$case_name" rti be uncompiled \
                    "$(grep -m1 'error:' "$w/cc-be.log" | tr '\t' ' ')"
                return 0; }
            local enc hex
            for enc in xcdr1 xcdr2; do
                if ! hex="$("$w/ref-be" be "${enc#xcdr}" | od -An -tx1 -v | tr -d ' \n')"; then
                    fail_row "$case_name" rti "$enc" be refused "RTI refused to serialise the sample"
                    continue
                fi
                printf '%s\trti\t%s\tbe\twrote\t%s\t\t%s\n' \
                    "$case_name" "$enc" "$((${#hex} / 2))" "$hex"
            done
        fi
        ;;
    esac
}

emit() {
    printf 'case\tvendor\tencoding\tendian\tstatus\tlength\theader\tbytes\n'
    local case_name vendor endian
    for case_name in ${cases//,/ }; do
        [ -f "$corpus/$case_name/type" ] || {
            echo "byte-gate: no case \`$case_name\` under $corpus" >&2; continue; }
        for vendor in $present; do
            for endian in ${endians//,/ }; do
                one "$case_name" "$vendor" "$endian"
            done
        done
    done
}

if [ -n "$out" ]; then emit > "$out"; else emit; fi

if ! vendor_report && [ "$require" -eq 1 ]; then
    echo "byte-gate: --require was given and a vendor was missing" >&2
    exit 1
fi
exit 0
