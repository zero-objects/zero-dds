#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# endpoint-sibling.sh — a hand-written endpoint SDK against the generated
# codec column on the same platform.
#
#   tools/interop-results/endpoint-sibling.sh --lang kotlin \
#       --sibling zerodds-java --out-root /tmp/endpoint-kotlin-out
#
# ## Why this comparison and not another
#
# Three of the hand-written bindings have a sibling among the generated
# columns that runs on the same runtime and reads the same clause:
# Kotlin beside Java on the JVM, F# beside C# on .NET, Node beside the
# TypeScript emitter. `endpoint-record.sh` already holds each against the
# Rust reference; this holds the two implementations against **each
# other**, over the cells where both wrote octets.
#
# A divergence here is the sharpest finding this tree can produce about a
# clause: two readings of the same paragraph, on the same platform, by
# two people who never called each other's code.
#
# ## What it needs
#
# `--out-root` is the directory `endpoint-record.sh` wrote its octets to.
# The sibling's octets come from the committed ledger,
# `tests/interop-results/<case>/<sibling>/<encoding>.<endian>.bin`; a
# sibling with no recording for a cell is counted as unrecorded and never
# as agreement.
set -uo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)" || exit 6

lang=""; sibling=""; out_root=""
while [ $# -gt 0 ]; do
    case "$1" in
        --lang) lang="$2"; shift 2 ;;
        --sibling) sibling="$2"; shift 2 ;;
        --out-root) out_root="$2"; shift 2 ;;
        --help|-h) sed -n '5,32p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "endpoint-sibling.sh: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done
[ -n "$lang" ] && [ -n "$sibling" ] && [ -d "$out_root" ] || {
    echo "endpoint-sibling.sh: --lang, --sibling and an existing --out-root are required" >&2
    exit 2
}

same=0; differ=0; unrecorded=0
for mine in "$out_root"/*/"endpoint-$lang"/*.bin; do
    [ -f "$mine" ] || continue
    cell=$(basename "$mine" .bin)
    case_name=$(basename "$(dirname "$(dirname "$mine")")")
    theirs="tests/interop-results/$case_name/$sibling/$cell.bin"
    if [ ! -f "$theirs" ]; then
        unrecorded=$((unrecorded+1))
        printf 'unrecorded\t%s\t%s\t%s has none\n' "$case_name" "$cell" "$sibling"
        continue
    fi
    if cmp -s "$mine" "$theirs"; then
        same=$((same+1))
    else
        differ=$((differ+1))
        printf 'different\t%s\t%s\n\t%-16s %s\n\t%-16s %s\n' \
            "$case_name" "$cell" \
            "endpoint-$lang" "$(od -An -tx1 -v "$mine" | tr -d ' \n')" \
            "$sibling" "$(od -An -tx1 -v "$theirs" | tr -d ' \n')"
    fi
done
printf '%s vs %s: same %d  different %d  (the sibling recorded none for %d)\n' \
    "endpoint-$lang" "$sibling" "$same" "$differ" "$unrecorded"
[ "$differ" -eq 0 ]
