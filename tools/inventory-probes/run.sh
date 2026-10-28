#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# run.sh — the three targeted probes behind the inventory rows that no
# corpus case can carry.
#
#   tools/inventory-probes/run.sh              all three
#   tools/inventory-probes/run.sh keyholder
#
#   keyholder   N3 and N7 — does the key holder reorder members by
#               ascending member id, and where does the MD5 branch start
#   bool-read   C2 and D1 — what each vendor's own deserialiser does with
#               a boolean octet that is neither 0 nor 1, and how wide
#               each front end makes a `char`
#   key-union   N11 — §7.2.2.4.4.4.8's own example, `union MyCommand
#               switch (@key CommandKind)`, through three front ends
#
# ## Why these are not corpus cases
#
# Each needs something a case directory cannot express. `keyholder`
# needs the *key holder*, which is not a payload: it reads
# `dds_stream_extract_keyBE_from_key`, RTI's
# `PRESTypePlugin_interpretedSerializeKeyForKeyhash` and Fast DDS's
# `compute_key`, three entry points no `--emit payload` reaches.
# `bool-read` needs a payload nobody's writer will produce — that is the
# whole question. `key-union` needs a type that three of the four front
# ends refuse to compile, so there is nothing to serialise and the
# refusal is the result.
#
# ## What this replaces
#
# `/root/n3-keyholder`, `/root/c2-boolread` and `/root/n11-keyunion` on
# the Linux test host, which `docs/wire-feature-inventory.md` cited by
# those paths.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

work="${INVENTORY_PROBE_WORK:-${TMPDIR:-/tmp}/inventory-probes}"
want="${*:-keyholder bool-read key-union}"

build_cyclone() { # <dir> <idl> <source> <binary-name>
    local d=$1 idl=$2 src=$3
    ( cd "$d" && "$CYCLONE_HOME/bin/idlc" "$(basename "$idl")" > idlc.log 2>&1 \
      && gcc -o probe "$(basename "$src")" "$(basename "${idl%.idl}").c" \
             -I. -I"$CYCLONE_HOME/include" -L"$CYCLONE_HOME/lib" -lddsc \
             -Wl,-rpath,"$CYCLONE_HOME/lib" > cc.log 2>&1 ) \
      || { echo "  the Cyclone probe did not build" >&2
           grep -m4 -i error "$d"/*.log >&2; return 1; }
}

build_rti() { # <dir> <idl> <source>
    local d=$1 idl=$2 src=$3 stem
    stem="$(basename "${idl%.idl}")"
    ( cd "$d" && "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" \
             -replace "$(basename "$idl")" > gen.log 2>&1 \
      && gcc -o probe "$(basename "$src")" "$stem.c" "${stem}Plugin.c" "${stem}Support.c" \
             -I. -I"$NDDSHOME/include" -I"$NDDSHOME/include/ndds" -L"$RTI_LIB" \
             -lnddsc -lnddscore -ldl -lm -lpthread -lrt -Wl,-rpath,"$RTI_LIB" \
             -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT > cc.log 2>&1 ) \
      || { echo "  the RTI probe did not build" >&2
           grep -m4 -i error "$d"/*.log >&2; return 1; }
}

build_fastdds() { # <dir> <idl> <source>
    local d=$1 idl=$2 src=$3 stem
    stem="$(basename "${idl%.idl}")"
    ( cd "$d" && "$FASTDDSGEN" -replace "$(basename "$idl")" > gen.log 2>&1 \
      && g++ -std=c++17 -o probe "$(basename "$src")" "${stem}PubSubTypes.cxx" \
             "${stem}TypeObjectSupport.cxx" -I. -I"$FASTDDS_HOME/include" \
             -L"$FASTDDS_HOME/lib" -lfastcdr -lfastdds \
             -Wl,-rpath,"$FASTDDS_HOME/lib" > cc.log 2>&1 ) \
      || { echo "  the Fast DDS probe did not build" >&2
           grep -m4 "error:" "$d/cc.log" >&2; return 1; }
}

# <group> <idl> — one working directory per (group, vendor), each with
# the IDL and that vendor's probe source in it.
stage() {
    local group=$1 vendor=$2 idl=$3 src=$4
    local d="$work/$group/$vendor"
    rm -rf "$d"; mkdir -p "$d"
    cp "$HERE/$group/$idl" "$d/"
    cp "$HERE/$group/$src" "$d/"
    printf '%s' "$d"
}

run_group() { # <group> <idl> <cyclone-src> <rti-src> <fastdds-src>
    local group=$1 idl=$2 c=$3 r=$4 f=$5 d
    echo "=========== $group ==========="
    for vendor in cyclone rti fastdds; do
        local src
        case "$vendor" in cyclone) src=$c ;; rti) src=$r ;; fastdds) src=$f ;; esac
        [ -n "$src" ] || continue
        if ! vendor_probe "$vendor"; then vendor_skip "$vendor" 1; continue; fi
        echo "--- $vendor"
        d="$(stage "$group" "$vendor" "$idl" "$src")"
        "build_$vendor" "$d" "$d/$idl" "$d/$src" || continue
        "$d/probe"
    done
}

for group in $want; do
    case "$group" in
        keyholder)
            run_group keyholder kh.idl probe-cyclone.c probe-rti.c probe-fastdds.cpp ;;
        bool-read)
            run_group bool-read gen.idl probe-cyclone.c probe-rti.c probe-fastdds.cpp ;;
        key-union)
            # No probe program: three of the four front ends refuse the
            # file, so there is nothing to serialise and the refusal is
            # the whole result.
            echo "=========== key-union ==========="
            for vendor in cyclone rti fastdds; do
                if ! vendor_probe "$vendor"; then vendor_skip "$vendor" 1; continue; fi
                d="$(stage key-union "$vendor" types.idl types.idl)"
                rc=0
                case "$vendor" in
                    cyclone) ( cd "$d" && "$CYCLONE_HOME/bin/idlc" types.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
                    rti)     ( cd "$d" && "$NDDSHOME/bin/rtiddsgen" -language C -platform "$RTI_TARGET" -replace types.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
                    fastdds) ( cd "$d" && "$FASTDDSGEN" -replace types.idl ) > "$d/fe.log" 2>&1 || rc=$? ;;
                esac
                printf '%-8s %-8s %s\n' "$vendor" \
                    "$([ "$rc" -eq 0 ] && echo ACCEPT || echo REJECT)" \
                    "$(sed 's/\x1b\[[0-9;]*m//g' "$d/fe.log" | grep -m1 -i 'error\|unsupported\|not supported' | tr '\n\t' '  ' | cut -c1-150)"
            done ;;
        *) echo "run: \`$group\` is not a group" >&2; exit 2 ;;
    esac
done

vendor_report || true
