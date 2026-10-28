#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# verify.sh — regenerate the cross-vendor read matrix and fail on a pair
# whose verdicts moved.
#
#   tools/pairwise-run/verify.sh --impls zerodds,cyclone,fastdds
#   tools/pairwise-run/verify.sh --update
#
# The recorded counts are
# `crates/ast-layout/tests/fixtures/vendor_roundtrip.rows.tsv`, one row
# per (encoding, byte order, writer, reader, verdict). The tables in
# `vendor_roundtrip.md` are sums over those rows.
#
# ## What is compared and what is not
#
# Only ordered pairs whose **both** ends ran. A run with three of the
# four implementations verifies nine of the sixteen pairs and says which
# seven it did not — it never treats a pair it could not measure as
# agreeing.
#
# ## Why a count and not a row
#
# The pairwise report has 5,024 rows per combination and its `detail`
# column carries a reader's own sentence, which changes on a reworded
# error message without any wire change. The verdict distribution per
# ordered pair is the thing that means something and the thing a
# regression moves.
#
# ## Cost
#
# Measured on the Linux test host, seven parallel jobs, over the four
# combinations: **50 minutes** for ZeroDDS, Cyclone and Fast DDS with a
# cold `WIREPROBE_CACHE` and the byte gate running beside it, and 67
# minutes for all four implementations with a warm cache. The cache is
# what decides it — the sixteen decode runs of one case differ only in
# the payload on stdin, and regenerating through `fastddsgen` for each
# spends the whole budget on the code generator.
#
# This does not belong in a per-commit job and the pipeline does not put
# it there.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
RECORD="${VENDOR_ROUNDTRIP_RECORD:-$ROOT/crates/ast-layout/tests/fixtures/vendor_roundtrip.rows.tsv}"

impls="${IMPLS:-zerodds,cyclone,rti,fastdds}"
update=0
require_asked=0
out_dir=""
reports_only=0
passthrough=()

while [ $# -gt 0 ]; do
    case "$1" in
        --impls) impls="$2"; shift 2 ;;
        --record) RECORD="$2"; shift 2 ;;
        --out-dir) out_dir="$2"; shift 2 ;;
        --reports-only) reports_only=1; shift ;;
        --update) update=1; shift ;;
        --require-asked) require_asked=1; passthrough+=(--require-asked); shift ;;
        --corpus|--jobs) passthrough+=("$1" "$2"); shift 2 ;;
        --help|-h) sed -n '5,35p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "verify: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done

if [ "$reports_only" -eq 1 ] && { [ -z "$out_dir" ] || [ "$update" -eq 1 ]; }; then
    echo "verify: reports-only requires an explicit report directory and cannot update the baseline" >&2
    exit 2
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
[ -n "$out_dir" ] || out_dir="$work/reports"

if [ "$reports_only" -eq 0 ]; then
    "$HERE/run.sh" --impls "$impls" --out-dir "$out_dir" --skip-dialects \
        "${passthrough[@]+"${passthrough[@]}"}" || exit "$?"
fi

fresh="$work/counts.tsv"
python3 - "$out_dir" > "$fresh" <<'PY'
import sys, pathlib, collections

# One row per (encoding, endian, writer, reader, verdict). The report's
# file name carries the encoding and byte order; its columns carry the
# rest. A row whose writer column is `-` is the writer's own failure
# line and has no reader, so it is not a pair.
counts = collections.Counter()
for path in sorted(pathlib.Path(sys.argv[1]).glob("report-*.tsv")):
    stem = path.name[len("report-"):-len(".tsv")]
    encoding, _, endian = stem.partition("-")
    if encoding == "dialects":
        continue
    with path.open(encoding="utf-8") as handle:
        handle.readline()
        for line in handle:
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 7 or parts[5] == "-":
                continue
            counts[(encoding, endian, parts[4], parts[5], parts[6])] += 1

print("encoding\tendian\twriter\treader\tverdict\tcount")
for key in sorted(counts):
    print("\t".join(key) + f"\t{counts[key]}")
PY
counted=$?

# The counting pass must not fail quietly. It once stopped on a report byte
# it could not decode, wrote nothing but a header, and `--update` copied that
# over the record: 129 rows gone, exit code 0. A run that did not count has
# nothing to record.
if [ "$counted" -ne 0 ]; then
    echo "verify: counting the reports failed (exit $counted); the record is untouched" >&2
    exit 1
fi
fresh_rows=$(($(wc -l < "$fresh") - 1))
if [ "$fresh_rows" -le 0 ]; then
    echo "verify: the run produced $fresh_rows rows; the record is untouched" >&2
    exit 1
fi

if [ "$update" -eq 1 ]; then
    mkdir -p "$(dirname "$RECORD")"
    cp "$fresh" "$RECORD"
    echo "verify: re-recorded $fresh_rows rows into $RECORD" >&2
    exit 0
fi

[ -f "$RECORD" ] || { echo "verify: no recorded counts at $RECORD" >&2; exit 2; }

python3 - "$RECORD" "$fresh" "$impls" "$require_asked" <<'PY'
import sys

recorded_path, fresh_path = sys.argv[1], sys.argv[2]
asked = sys.argv[3].split(",")
require_asked = sys.argv[4] == "1"


def load(path):
    rows = {}
    with open(path, encoding="utf-8") as handle:
        handle.readline()
        for line in handle:
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 6:
                rows[tuple(parts[:5])] = int(parts[5])
    return rows


recorded = load(recorded_path)
fresh = load(fresh_path)

# An implementation ran if it appears as a writer in the fresh counts.
# A pair is verifiable only when both of its ends ran, which is the
# whole asymmetry a partial run has to be honest about.
ran = {key[2] for key in fresh} | {key[3] for key in fresh}
# An implementation asked for that neither wrote nor read is a missing
# toolchain, not a result. `--require-asked` makes that a failure, so a
# job pointed at an image without the vendors cannot compare nothing and
# go green.
never_ran = [impl for impl in asked if impl not in ran]
for impl in never_ran:
    print(f"asked for {impl}, which wrote and read nothing", file=sys.stderr)

changed, unverified = [], set()
for key, was in recorded.items():
    if key[2] not in ran or key[3] not in ran:
        unverified.add((key[2], key[3]))
        continue
    now = fresh.get(key, 0)
    if now != was:
        changed.append((key, was, now))

appeared = [(key, value) for key, value in fresh.items() if key not in recorded]

for pair in sorted(unverified):
    print(f"NOT VERIFIED: {pair[0]} -> {pair[1]} — one end did not run", file=sys.stderr)

for key, was, now in changed:
    print(f"CHANGED {'  '.join(key)}: recorded {was}, regenerated {now}", file=sys.stderr)

for key, now in appeared:
    print(f"NEW {'  '.join(key)}: {now} rows, the record has no such verdict",
          file=sys.stderr)

pairs = {(key[2], key[3]) for key in recorded}
print(f"{len(pairs) - len(unverified)} of {len(pairs)} ordered pairs verified, "
      f"{len(changed)} counts changed, {len(appeared)} new", file=sys.stderr)
if require_asked and never_ran:
    print(f"--require-asked: {', '.join(never_ran)} did not run", file=sys.stderr)

sys.exit(1 if changed or appeared or (require_asked and never_ran) else 0)
PY
