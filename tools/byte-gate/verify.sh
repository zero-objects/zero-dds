#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# verify.sh — regenerate the vendor byte rows and fail on any that
# changed.
#
#   tools/byte-gate/verify.sh                        every vendor present
#   tools/byte-gate/verify.sh --vendors cyclone,fastdds,opendds
#   tools/byte-gate/verify.sh --update               re-record instead
#
# The recorded rows are
# `crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv`, which is the
# machine-readable form of what
# `crates/ast-layout/tests/fixtures/vendor_bytes.md` narrates. A run
# regenerates them from the two case sets and compares.
#
# ## Why comparing is the whole point
#
# A job that produces a report says a run happened. A job that compares
# says the answer did not move. The vendor bytes are the foundation
# every layout rule in this project was derived from: if Cyclone 11.0.2
# starts writing a different delimiter, that is not a report to read
# later, it is a failure now.
#
# ## What a run does not verify
#
# Whatever vendor is not installed. Named, with the row count, on
# stderr. A pipeline without RTI is expected to leave the RTI rows
# unverified and to say so; it is not expected to pass quietly.
#
# `--require-all` makes any unverified row a failure. `--require-asked`
# is the narrower one a job wants: it fails when a vendor named in
# `--vendors` produced nothing at all, which is what a toolchain missing
# from the image looks like. Without it a job pointed at an image that
# has none of the three would compare nothing, find nothing changed, and
# go green.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
RECORD="${VENDOR_BYTES_RECORD:-$ROOT/crates/ast-layout/tests/fixtures/vendor_bytes.rows.tsv}"

vendors="cyclone,rti,fastdds,opendds"
endians="le,be"
# `--cases` is for a mutation check and for a single case under
# investigation. A partial run compares only the rows it produced, so
# every case it did not run is counted as unverified rather than as
# agreeing.
cases=""
update=0
require_all=0
require_asked=0
keep=""

while [ $# -gt 0 ]; do
    case "$1" in
        --vendors) vendors="$2"; shift 2 ;;
        --endians) endians="$2"; shift 2 ;;
        --cases) cases="$2"; shift 2 ;;
        --record) RECORD="$2"; shift 2 ;;
        --regenerated-out) keep="$2"; shift 2 ;;
        --update) update=1; shift ;;
        --require-all) require_all=1; shift ;;
        --require-asked) require_asked=1; shift ;;
        --help|-h) sed -n '5,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "verify: \`$1\` is not an option" >&2; exit 2 ;;
    esac
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
fresh="$work/regenerated.tsv"

# Both case sets, one report. The `set` column keeps them apart so a
# case name that exists in both could never be silently merged.
case_args=()
[ -n "$cases" ] && case_args=(--cases "$cases")
{
    "$HERE/gate.sh" --vendors "$vendors" --endians "$endians" \
        "${case_args[@]+"${case_args[@]}"}" \
        | awk -F'\t' 'BEGIN{OFS="\t"} NR==1{print "set",$0; next} {print "byte-gate",$0}'
    "$ROOT/tools/wire-oracle/gate.sh" --vendors "$vendors" --endians "$endians" \
        "${case_args[@]+"${case_args[@]}"}" \
        | awk -F'\t' 'BEGIN{OFS="\t"} NR==1{next} {print "wire-oracle",$0}'
} > "$fresh"

[ -n "$keep" ] && cp "$fresh" "$keep"

# A row the run did not produce because its case was not asked for is
# unverified and not changed, which the comparison below already gets
# right by keying on the whole tuple.
# `--update` writes the run's rows over the record, so a run that was
# narrowed with `--cases` or `--vendors` would drop every row it did not
# produce — silently, because the file it writes is well-formed and the
# next `--verify` would then compare a truncated record and pass. A
# re-record has to be the full sweep; a narrowed one is refused here,
# with the count it would have deleted, rather than performed.
if [ "$update" -eq 1 ]; then
    [ -f "$RECORD" ] && recorded_rows=$(($(wc -l < "$RECORD") - 1)) || recorded_rows=0
    fresh_rows=$(($(wc -l < "$fresh") - 1))
    if [ -n "$cases" ] || [ "$fresh_rows" -lt "$recorded_rows" ]; then
        printf 'verify: refusing to re-record %s rows over a record of %s.\n' \
            "$fresh_rows" "$recorded_rows" >&2
        printf '  --update replaces the whole file; this run produced %s fewer row(s)%s.\n' \
            "$((recorded_rows - fresh_rows))" \
            "${cases:+ (--cases $cases)}" >&2
        printf '  Re-record with the full sweep: every case, and every vendor whose\n' >&2
        printf '  rows are in the record — a vendor that cannot run writes "refused"\n' >&2
        printf '  over the octets it recorded when it could.\n' >&2
        exit 2
    fi
    mkdir -p "$(dirname "$RECORD")"
    cp "$fresh" "$RECORD"
    echo "verify: re-recorded $fresh_rows rows into $RECORD" >&2
    exit 0
fi

[ -f "$RECORD" ] || { echo "verify: no recorded rows at $RECORD" >&2; exit 2; }

# The comparison is per row and keyed on (set, case, vendor, encoding,
# endian). A vendor that did not run contributes no regenerated row, so
# its recorded rows are counted as unverified rather than as missing —
# the two are different findings and only one of them is a failure.
python3 - "$RECORD" "$fresh" "$vendors" "$require_all" "$cases" "$require_asked" <<'PY'
import re
import sys

record_path, fresh_path = sys.argv[1], sys.argv[2]
asked = sys.argv[3].split(",")
require_all = sys.argv[4] == "1"
# A run restricted to some cases verifies those and leaves the rest
# unverified. Without this the rows it did not ask for would read as a
# vendor that stopped answering, which is what an empty `--cases` run
# genuinely means and a filtered one does not.
only_cases = {c for c in sys.argv[5].split(",") if c}
require_asked = sys.argv[6] == "1"


# The work directory is not part of the measurement.
#
# A front end names the file it was handed, and Cyclone's `idlc` names it
# absolutely, so `gate.sh`'s `--work` / `BYTE_GATE_WORK` — a run
# parameter — ended up inside the `bytes` column of every `ungenerated`
# row it wrote. 72 recorded rows carry
# `/tmp/byte-gate-work/<case>/<vendor>-<endian>/gen.idl:…`, which is the
# directory of the host that recorded them. Comparing that text compares
# where the run happened: the same `idlc`, the same IDL and the same
# diagnostic read as 72 changed rows on every run that set the variable
# somewhere else, and the octet columns were identical in all 72.
#
# So the field is normalised on both sides before the comparison, and
# narrowly: only a `<absolute dir>/<case>/<vendor>-<le|be>/` prefix goes,
# built from the row's own key. Anything else the vendor said — the line,
# the column, the sentence — still has to match exactly.
def strip_work_dir(key, values):
    _, case, vendor, _, _ = key
    pattern = re.compile(
        r"/\S*/" + re.escape(case) + r"/" + re.escape(vendor) + r"-(?:le|be)/"
    )
    return [pattern.sub("", value) for value in values]


def load(path):
    rows = {}
    with open(path, encoding="utf-8") as handle:
        header = handle.readline().rstrip("\n").split("\t")
        for line in handle:
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 5:
                continue
            while len(parts) < len(header):
                parts.append("")
            key = tuple(parts[:5])
            rows[key] = strip_work_dir(key, parts[5:])
    return rows


recorded = load(record_path)
fresh = load(fresh_path)

# Which vendors actually ran is read out of the fresh report and not
# out of `--vendors`: a vendor that was asked for and is not installed
# produces no rows, and its recorded rows are unverified rather than
# changed. Reading the request instead of the result would turn every
# absent toolchain into a wall of false failures.
ran = {key[2] for key in fresh}
# A vendor that was asked for and produced nothing is the shape a missing
# toolchain takes. Under `--require-asked` that is a failure and not a
# note: a job configured to verify three vendors and finding none of them
# installed has verified nothing, and going green on it would be exactly
# the silent pass this gate exists to prevent.
never_ran = [vendor for vendor in asked if vendor not in ran]
for vendor in never_ran:
    print(f"asked for {vendor}, which produced no rows at all", file=sys.stderr)
changed, appeared = [], []
# The two reasons a row goes unverified are kept apart, because only one
# of them is about a toolchain: `absent` is a vendor that did not run,
# `unasked` is a case this run was told to leave out.
absent, unasked = [], []

for key, value in recorded.items():
    if only_cases and key[1] not in only_cases:
        unasked.append(key)
        continue
    if key[2] not in ran:
        absent.append(key)
        continue
    if key not in fresh:
        # The vendor was asked for and produced nothing at all for this
        # row: a toolchain that is present but no longer answers is a
        # change and not an absence.
        changed.append((key, value, ["<no row>"]))
    elif fresh[key] != value:
        changed.append((key, value, fresh[key]))

for key in fresh:
    if key not in recorded:
        appeared.append((key, fresh[key]))

def name(key):
    return "  ".join(key)

per = {}
for key in absent:
    per[key[2]] = per.get(key[2], 0) + 1
for vendor, count in sorted(per.items()):
    print(f"NOT VERIFIED: {vendor} — {count} recorded rows; "
          f"its toolchain produced nothing in this run", file=sys.stderr)
if unasked:
    print(f"not asked for: {len(unasked)} recorded rows in cases outside --cases",
          file=sys.stderr)

for key, was, now in changed:
    print(f"CHANGED {name(key)}", file=sys.stderr)
    print(f"  recorded    {'  '.join(was)}", file=sys.stderr)
    print(f"  regenerated {'  '.join(now)}", file=sys.stderr)

for key, now in appeared:
    print(f"NEW ROW {name(key)} — {'  '.join(now)}", file=sys.stderr)
    print("  the record has no such row; re-record with "
          "tools/byte-gate/verify.sh --update", file=sys.stderr)

verified = len(recorded) - len(absent) - len(unasked) - len(changed)
print(f"verified {verified} rows, {len(changed)} changed, {len(appeared)} new, "
      f"{len(absent)} not verified, {len(unasked)} not asked for", file=sys.stderr)
# `--require-all` is for the run that is supposed to be complete: it
# turns an unverified row into a failure instead of a note, so a job
# that is meant to cover all four vendors cannot go green on two.
missing = sorted({key[2] for key in absent}) if require_all else []
for vendor in missing:
    print(f"--require-all: {vendor} did not run", file=sys.stderr)

if require_asked and never_ran:
    print(f"--require-asked: {', '.join(never_ran)} did not run — "
          f"the toolchain is missing where this run expected it",
          file=sys.stderr)

sys.exit(1 if changed or appeared or missing or (require_asked and never_ran) else 0)
PY
exit $?
