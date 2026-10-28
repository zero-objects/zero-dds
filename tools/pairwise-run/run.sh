#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# pairwise-run — the whole cross-vendor read matrix, in one command.
#
# `tools/wireprobe/wireprobe-pairwise` measures one (encoding, byte
# order) combination. This runs the four combinations that
# `crates/ast-layout/tests/fixtures/vendor_roundtrip.md` records, then
# the dialect sweep underneath it, and leaves one TSV per combination.
#
#   tools/pairwise-run/run.sh                     every combination
#   IMPLS=zerodds,cyclone,fastdds tools/pairwise-run/run.sh --out-dir out
#
#   --out-dir <dir>   where the TSVs go; default ./pairwise-out
#   --corpus <dir>    default tests/interop-corpus in this checkout
#   --impls <list>    default zerodds,cyclone,rti,fastdds
#   --jobs <n>        cases in parallel; default the core count
#   --policy <name>   the writer setting for the four combinations;
#                     default `none`
#   --skip-dialects   the four combinations only, not the dialect sweep
#
# ## Why the default policy is `none`
#
# Every foreign probe refuses a `--dialect` that is not its own — it
# writes what its vendor writes and will not pretend otherwise. So a run
# that mixes implementations and names a dialect gets `unwritten` for
# every column but one, which reads as a vendor that produced nothing
# and is really a harness that asked it the wrong question.
#
# `none` passes no `--dialect` at all and each probe answers with its own
# defaults, which is what a run comparing implementations against each
# other wants. `/root/p2-run/run.sh` passed `--policies cyclone` and
# predates the probes' refusal; a run of it today would silently lose
# three of the four columns.
#
# ## Cost
#
# The full four-combination run is 20,096 rows and compiles a probe per
# (case, vendor, direction). On the Linux test host with a warm
# `WIREPROBE_CACHE` it is minutes; cold it is hours, and the cache is
# what makes the difference — set `WIREPROBE_CACHE` to something
# persistent or pay the compile again.
#
# ## What this replaces
#
# `/root/p2-run/run.sh`, which hard-coded a checkout at
# `/root/zerodds-p2` and a cache at `/root/p2-run/cache`.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../interop-vendors.sh
. "$ROOT/tools/interop-vendors.sh"

out_dir="./pairwise-out"
jobs="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
skip_dialects=0
cases=""
require_asked=0
impls="${IMPLS:-zerodds,cyclone,rti,fastdds}"
corpus="${INTEROP_CORPUS:-$ROOT/tests/interop-corpus}"
policy="${PAIRWISE_POLICY:-none}"

while [ $# -gt 0 ]; do
    case "$1" in
        --out-dir) out_dir="$2"; shift 2 ;;
        --corpus) corpus="$2"; shift 2 ;;
        --policy) policy="$2"; shift 2 ;;
        --jobs) jobs="$2"; shift 2 ;;
        --cases) cases="$2"; shift 2 ;;
        --require-asked) require_asked=1; shift ;;
        --impls) impls="$2"; shift 2 ;;
        --skip-dialects) skip_dialects=1; shift ;;
        --help|-h) sed -n '5,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "pairwise-run: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done

mkdir -p "$out_dir"
export WIREPROBE_CACHE="${WIREPROBE_CACHE:-${TMPDIR:-/tmp}/wireprobe-cache}"
mkdir -p "$WIREPROBE_CACHE"

# Absence is named before the run and not discovered as a hole in the
# report: a matrix missing an implementation has fewer rows, and fewer
# rows read as a smaller matrix rather than as a missing column.
for impl in ${impls//,/ }; do
    vendor_probe "$impl" || vendor_skip "$impl" 5024
done
if [ "$require_asked" -eq 1 ]; then vendor_report || exit 125; fi
case_args=()
[ -z "$cases" ] || case_args=(--cases "$cases")

for combo in "xcdr2 le" "xcdr2 be" "xcdr1 le" "xcdr1 be"; do
    set -- $combo
    echo "=== $1 $2 started $(date -Is) ===" >&2
    "$ROOT/tools/wireprobe/wireprobe-pairwise" --encodings "$1" --endians "$2" \
        --jobs "$jobs" --impls "$impls" --policies "$policy" --corpus "$corpus" \
        "${case_args[@]+"${case_args[@]}"}" --out "$out_dir/report-$1-$2.tsv" || exit "$?"
    echo "=== $1 $2 done $(date -Is), $(wc -l < "$out_dir/report-$1-$2.tsv") rows ===" >&2
done

if [ "$skip_dialects" -eq 0 ]; then
    echo "=== dialect sweep started $(date -Is) ===" >&2
    "$ROOT/tools/wireprobe/wireprobe-pairwise" --encodings xcdr2 --endians le \
        --jobs "$jobs" --impls "$impls" --corpus "$corpus" \
        --policies cyclone,rti,fastdds,specification \
        "${case_args[@]+"${case_args[@]}"}" --out "$out_dir/report-dialects.tsv" || exit "$?"
    echo "=== dialect sweep done $(date -Is), $(wc -l < "$out_dir/report-dialects.tsv") rows ===" >&2
fi

vendor_report || true
echo "ALL DONE $(date -Is)" >&2
