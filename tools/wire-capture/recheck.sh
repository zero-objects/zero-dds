#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Every row of the wire table, re-derived from the committed frames.
#
#   tools/wire-capture/recheck.sh tools/wire-capture/frames > rows.tsv
#
# The captures themselves are not in the repository — a loopback `.pcap`
# of a DDS run is mostly discovery and ACKNACKs, and there are 200 of
# them. What is committed is the octets that carried a sample: every UDP
# payload holding a user `DATA`, plus the SEDP announcements that say
# which writer served which topic, and for the ORBs the reassembled GIOP
# streams. From those, `extract-*.py` and `check-run.py` produce the
# same rows the run produced, on a machine with no vendor installed and
# no `tshark`.
#
# That is the property this script exists to check. It takes no host, no
# toolchain and no network: if it prints rows that differ from
# `tests/interop-results/wire-goldens.tsv`, either the frames or the
# table has moved.
#
# It cannot re-derive the two checks that are properties of the *run*
# and not of the octets — that the subscriber read every sample, and
# that the capture was not truncated. Those were made when the run
# happened and are in `logs/`.
set -uo pipefail

TOOLS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
frames="${1:-$TOOLS/frames}"
count="${ZD_WIRE_COUNT:-6}"

[ -d "$frames" ] || { echo "recheck: $frames is not a directory" >&2; exit 2; }

for impl_dir in "$frames"/*/; do
    impl="$(basename "$impl_dir")"
    case "$impl" in
        omniorb|tao|jacorb) extract="extract-giop.py"; giop=1 ;;
        *) extract="extract-rtps.py"; giop=0 ;;
    esac
    for file in "$impl_dir"*.frames; do
        [ -e "$file" ] || continue
        base="$(basename "$file" .frames)"
        case_name="${base%.*}"
        version="${base##*.xcdr}"
        control=$(awk -F'\t' -v c="$case_name" '$1 == c { print $3 }' \
                  "$TOOLS/cases.tsv")
        work="$(mktemp -d)"
        if [ "$giop" -eq 1 ]; then
            python3 "$TOOLS/$extract" "$file" --from-frames --operation put \
                > "$work/rows.tsv" 2>/dev/null
        else
            python3 "$TOOLS/$extract" "$file" --from-frames \
                > "$work/rows.tsv" 2>/dev/null
        fi
        if [ "$giop" -eq 1 ]; then
            python3 "$TOOLS/check-run.py" --rows "$work/rows.tsv" --count "$count" \
                --impl "$impl" --case "$case_name" --version "$version" \
                --control "${control:-equal}" --giop
        else
            python3 "$TOOLS/check-run.py" --rows "$work/rows.tsv" --count "$count" \
                --impl "$impl" --case "$case_name" --version "$version" \
                --control "${control:-equal}" --topic "zdw_${case_name//-/_}"
        fi
        rm -rf "$work"
    done
done
