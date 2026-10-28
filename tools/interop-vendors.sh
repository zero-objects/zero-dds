#!/bin/bash
# interop-vendors.sh — where each foreign toolchain is, and what to say
# when it is not there.
#
# Sourced by every interop harness in `tools/`. It exists because the
# harnesses have to run in three places that put the vendors in three
# different directories: the Linux test host, the pinned `ci-interop`
# image, and a developer's checkout with two of nine installed. Hard
# coding `/opt/cyclone` made a harness that ran in exactly one of them.
#
# Every location is an environment variable with the test host's layout
# as its default, so a run on the host needs no setting at all and a run
# anywhere else needs one line per vendor.
#
# ## The rule about absence
#
# A missing toolchain is never a pass and never a silent skip. It is
# reported by name, with the number of rows the run did not verify:
#
#   vendor_skip rti 47   ->   "rti: NOT PRESENT (NDDSHOME=... ), 47 rows
#                              not verified in this run"
#
# A harness that stayed quiet about an absent vendor would let a green
# job stand for a matrix it never measured, which is the failure this
# whole gate exists to prevent.

# ---------------------------------------------------------------------
# Locations. The names match the ones `tools/wireprobe/` already uses;
# a harness and a probe asked to find the same toolchain read the same
# variable.
# ---------------------------------------------------------------------
CYCLONE_HOME="${CYCLONE_HOME:-/opt/cyclone}"
NDDSHOME="${NDDSHOME:-/opt/rti.com/rti_connext_dds-7.7.0}"
RTI_LIB="${RTI_LIB:-$NDDSHOME/lib/x64Linux4gcc8.5.0}"
RTI_TARGET="${RTI_TARGET:-x64Linux4gcc7.3.0}"
FASTDDS_HOME="${FASTDDS_HOME:-/opt/fastdds}"
OPENDDS_HOME="${OPENDDS_HOME:-/opt/opendds}"
TAO_HOME="${TAO_HOME:-$OPENDDS_HOME}"
TAO_IDL="${TAO_IDL:-$TAO_HOME/bin/tao_idl}"
OMNIIDL="${OMNIIDL:-omniidl}"
OMNIORB_INCLUDE="${OMNIORB_INCLUDE:-/usr/include/omniORB4}"
JACORB_HOME="${JACORB_HOME:-/opt/jacorb}"
ROS_PREFIX="${ROS_PREFIX:-/root/micromamba/envs/ros2}"
# HDDS is the second pure-Rust DDS: its generator is a single binary the
# test host builds from source, and its column's octets come out of that
# generator's own `encode_*` output — `tools/wireprobe/wireprobe-hdds`.
HDDSGEN="${HDDSGEN:-/root/hdds-probe/hdds_gen_src/target/release/hddsgen}"
# OpenSplice Community Edition 6.9, built from source by
# `tools/opensplice/build-opensplice.sh`; `wireprobe-opensplice` reads
# the same variable.
OSPL_HOME="${OSPL_HOME:-/opt/opensplice-6.9}"

# `fastddsgen` is the one that genuinely moves: the test host builds it
# under /root/vendors and the ci-interop image installs it under /opt.
# Searched in order rather than defaulted, so neither location has to
# know about the other.
if [ -z "${FASTDDSGEN:-}" ]; then
    for _fdg in /opt/fastddsgen/scripts/fastddsgen \
                /root/vendors/fastddsgen/scripts/fastddsgen \
                "$(command -v fastddsgen 2>/dev/null || true)"; do
        if [ -n "$_fdg" ] && [ -x "$_fdg" ]; then FASTDDSGEN="$_fdg"; break; fi
    done
    FASTDDSGEN="${FASTDDSGEN:-/root/vendors/fastddsgen/scripts/fastddsgen}"
    unset _fdg
fi

export CYCLONE_HOME NDDSHOME RTI_LIB RTI_TARGET FASTDDS_HOME OPENDDS_HOME
export TAO_HOME TAO_IDL OMNIIDL OMNIORB_INCLUDE JACORB_HOME ROS_PREFIX FASTDDSGEN
export HDDSGEN OSPL_HOME

# OpenDDS's generators read these three; setting them here keeps every
# caller from repeating the same three lines.
export DDS_ROOT="${DDS_ROOT:-$OPENDDS_HOME/share/dds}"
export ACE_ROOT="${ACE_ROOT:-$OPENDDS_HOME/share/ace}"
export TAO_ROOT="${TAO_ROOT:-$OPENDDS_HOME/share/tao}"

# ---------------------------------------------------------------------
# Presence. One test per vendor, against the exact file the harness will
# execute — not against a directory, because an empty prefix directory
# is the shape a half-finished install takes.
# ---------------------------------------------------------------------

# vendor_probe <name> -> 0 if the toolchain can run, 1 otherwise
vendor_probe() {
    case "$1" in
        cyclone)
            # The header is tested and not only `idlc`: the CycloneDDS
            # Python wheel ships `idlc` and `libddsc` and no headers at
            # all, so a prefix that passes the binary test alone still
            # cannot compile a probe.
            [ -x "$CYCLONE_HOME/bin/idlc" ] \
                && [ -f "$CYCLONE_HOME/include/dds/cdr/dds_cdrstream.h" ] ;;
        rti)      [ -x "$NDDSHOME/bin/rtiddsgen" ] && [ -d "$RTI_LIB" ] ;;
        fastdds)  [ -x "$FASTDDSGEN" ] && [ -d "$FASTDDS_HOME/include" ] ;;
        opendds)  [ -x "$OPENDDS_HOME/bin/opendds_idl" ] && [ -x "$TAO_IDL" ] ;;
        omniorb)  command -v "$OMNIIDL" >/dev/null 2>&1 \
                      && [ -d "$OMNIORB_INCLUDE" ] ;;
        tao)      [ -x "$TAO_IDL" ] ;;
        jacorb)   [ -d "$JACORB_HOME" ] && command -v java >/dev/null 2>&1 ;;
        ros)      [ -d "$ROS_PREFIX" ] ;;
        # The generator and a Rust toolchain: the probe compiles
        # hddsgen's own output, so both have to be there.
        hdds)     [ -x "$HDDSGEN" ] && command -v cargo >/dev/null 2>&1 ;;
        # idlpp and the SACPP library the probe links.
        opensplice) [ -x "$OSPL_HOME/bin/idlpp" ] && [ -f "$OSPL_HOME/lib/libdcpssacpp.so" ] ;;
        zerodds)  return 0 ;;
        *)        return 1 ;;
    esac
}

# vendor_where <name> -> the setting a reader has to change to fix it
vendor_where() {
    case "$1" in
        cyclone)  printf 'CYCLONE_HOME=%s'  "$CYCLONE_HOME" ;;
        rti)      printf 'NDDSHOME=%s'      "$NDDSHOME" ;;
        fastdds)  printf 'FASTDDSGEN=%s FASTDDS_HOME=%s' "$FASTDDSGEN" "$FASTDDS_HOME" ;;
        opendds)  printf 'OPENDDS_HOME=%s'  "$OPENDDS_HOME" ;;
        omniorb)  printf 'OMNIIDL=%s OMNIORB_INCLUDE=%s' "$OMNIIDL" "$OMNIORB_INCLUDE" ;;
        tao)      printf 'TAO_IDL=%s'       "$TAO_IDL" ;;
        jacorb)   printf 'JACORB_HOME=%s'   "$JACORB_HOME" ;;
        ros)      printf 'ROS_PREFIX=%s'    "$ROS_PREFIX" ;;
        hdds)     printf 'HDDSGEN=%s'       "$HDDSGEN" ;;
        opensplice) printf 'OSPL_HOME=%s'   "$OSPL_HOME" ;;
        *)        printf 'unknown vendor'   ;;
    esac
}

# vendor_skip <name> [rows] — the loud form. Writes to stderr so a run
# whose stdout is a TSV report keeps the report parseable, and counts
# into VENDOR_SKIPPED so the caller can summarise at the end.
VENDOR_SKIPPED="${VENDOR_SKIPPED:-}"
VENDOR_SKIPPED_ROWS="${VENDOR_SKIPPED_ROWS:-0}"
vendor_skip() {
    local name="$1" rows="${2:-0}"
    printf '%s: NOT PRESENT (%s) — %s rows not verified in this run\n' \
        "$name" "$(vendor_where "$name")" "$rows" >&2
    VENDOR_SKIPPED="$VENDOR_SKIPPED $name"
    VENDOR_SKIPPED_ROWS=$((VENDOR_SKIPPED_ROWS + rows))
}

# vendor_report — the closing summary. Prints what ran and what did not,
# and returns 1 when anything was skipped so a caller that wants a
# complete matrix can turn absence into a failure.
vendor_report() {
    if [ -z "${VENDOR_SKIPPED// /}" ]; then
        printf 'every requested vendor was present\n' >&2
        return 0
    fi
    printf 'NOT VERIFIED IN THIS RUN:%s — %s rows\n' \
        "$VENDOR_SKIPPED" "$VENDOR_SKIPPED_ROWS" >&2
    return 1
}
