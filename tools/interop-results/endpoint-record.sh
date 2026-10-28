#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# endpoint-record.sh — one hand-written endpoint SDK against the whole
# corpus, in all ten configurations, with the octets kept.
#
#   tools/interop-results/endpoint-record.sh --lang zig
#   tools/interop-results/endpoint-record.sh --lang ada-native --jobs 8 \
#       --tsv /tmp/ada-native.tsv
#
# ## What it answers
#
# Three numbers per language, not one:
#
#   reached      the (case, encoding, endian) cells the binding's write
#                surface can express at all
#   same         those of them whose octets equal
#                tests/interop-results/<case>/zerodds-rust/<enc>.<endian>.bin
#   different    those that do not — each one a finding
#
# A cell the surface cannot express is counted apart, under `no-form`,
# because it is the extent of the binding and not a failure of it.
# `docs/specs/zerodds-verdicts-1.0.md` gives the vocabulary and the exit
# codes the probe answers in.
#
# A host without the binding's compiler produces **no numbers at all**.
# The probe answers exit 9, `toolchain-absent`, the sweep writes no cell
# and leaves `--tsv` as it found it, and this exits with that code before
# it counts anything — the three numbers below would otherwise read
# `reached 0  same 0  different 0` and the run would end 0, a green line
# for a column nobody measured. It used to be worse than that: a missing
# compiler reached `endpoint_build` as an ordinary build failure and
# every cell of the column was recorded `generator-defect`.
#
# ## What it compares against
#
# The recorded Rust octets and nothing else. No constant written into
# this script, no expectation table: a third opinion about the wire is
# how two columns come to agree with each other and with nothing real.
#
# ## Why this is nine lines and not a sweep
#
# It used to be its own "generate, compile, run, compare" loop beside
# `tools/wireprobe/wireprobe-endpoint-corpus`, which is the same loop.
# Two readings of one comparison drift, and the drift is invisible
# because both report green. There is one sweep; this is the ledger's
# arguments to it and the ledger's summary of the rows it wrote.
#
# ## What it does not touch
#
# `tests/interop-results/`. The octets go under `--out-root`, which
# defaults to a scratch directory, so a measurement run cannot rewrite
# the committed ledger.
set -uo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)" || exit 2

lang=""
jobs="$( (nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4) )"
out_root=""
tsv=""
only=""

while [ $# -gt 0 ]; do
    case "$1" in
        --lang) lang="$2"; shift 2 ;;
        --jobs) jobs="$2"; shift 2 ;;
        --out-root) out_root="$2"; shift 2 ;;
        --tsv) tsv="$2"; shift 2 ;;
        --cases) only="$2"; shift 2 ;;
        --help|-h) sed -n '5,54p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "endpoint-record.sh: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done
[ -n "$lang" ] || { echo "endpoint-record.sh: --lang is required" >&2; exit 2; }
[ -x "tools/wireprobe/wireprobe-endpoint-$lang" ] \
    || { echo "endpoint-record.sh: no probe for \`$lang\`" >&2; exit 2; }
[ -n "$out_root" ] || out_root="$(mktemp -d)/endpoint-$lang"
[ -n "$tsv" ] || tsv="$out_root.tsv"
mkdir -p "$out_root"

set -- --langs "$lang" --jobs "$jobs" --out "$tsv" --octets "$out_root"
[ -n "$only" ] && set -- "$@" --cases "$only"
tools/wireprobe/wireprobe-endpoint-corpus "$@" >&2 || exit $?

# The rows the sweep wrote, counted the way the ledger reads them. A
# `noref` cell is outside every denominator: the reference column wrote
# no octets for it either.
reached=$(awk -F'\t' '$5 == "same" || $5 == "different"' "$tsv" | wc -l | tr -d ' ')
same=$(awk -F'\t' '$5 == "same"' "$tsv" | wc -l | tr -d ' ')
different=$(awk -F'\t' '$5 == "different"' "$tsv" | wc -l | tr -d ' ')
no_form=$(awk -F'\t' '$5 == "no-form"' "$tsv" | wc -l | tr -d ' ')
gap=$(awk -F'\t' \
    '$5 == "harness-gap" || $5 == "generator-defect" || $5 == "refused" || $5 == "noref"' \
    "$tsv" | wc -l | tr -d ' ')

printf '%-12s reached %4d  same %4d  different %4d   (no-form %4d, unmeasured %3d)\n' \
    "$lang" "$reached" "$same" "$different" "$no_form" "$gap"
echo "rows: $tsv" >&2
echo "octets: $out_root" >&2
[ "$different" -eq 0 ]
