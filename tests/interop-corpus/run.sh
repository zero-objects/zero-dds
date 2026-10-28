#!/bin/sh
# Every case in the corpus through the Rust probe, one line each.
#
#   ./run.sh                     all four {xcdr1, xcdr2} x {le, be}
#   ./run.sh xcdr1 be            one combination
#
# The gate is `cargo test -p zerodds-wire-probe --test interop_corpus`,
# which walks the same corpus in process and compares every case against
# the bytes its README records. This script is the same walk for a human:
# it prints the verdicts rather than asserting them, which is what makes
# it useful when a case has just changed and the recorded bytes have not.
#
# The default is all four combinations because one is not the corpus.
# Under `xcdr2 le` alone the PL_CDR1 extended parameter header and both
# big-endian byte-swap arms of the writer never execute
# (docs/codec-coverage-report.md, the B -> C delta).
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)

probe="$root/target/debug/wireprobe-zerodds"
if [ ! -x "$probe" ]; then
    cargo build -q --manifest-path "$root/Cargo.toml" \
        -p zerodds-wire-probe --bin wireprobe-zerodds || exit 1
fi

# One combination when the caller names one, otherwise all four.
if [ $# -gt 0 ]; then
    modes="${1:-xcdr2}:${2:-le}"
else
    modes="xcdr2:le xcdr2:be xcdr1:le xcdr1:be"
fi

total=0
for mode in $modes; do
    encoding=${mode%:*}
    endian=${mode#*:}
    printf '=== %s %s ===\n' "$encoding" "$endian"
    failed=0
    for dir in "$here"/*/; do
        case_name=$(basename "$dir")
        [ -f "$dir/type" ] || continue
        out=$("$probe" --idl "$dir/type.idl" --type "$(cat "$dir/type")" \
                       --values "$dir/values.json" \
                       --encoding "$encoding" --endian "$endian" 2>&1)
        if [ $? -eq 0 ]; then
            printf '%-52s ok    %s\n' "$case_name" "$out"
        else
            printf '%-52s FAIL  %s\n' "$case_name" "$out"
            failed=$((failed + 1))
        fi
    done
    printf '\n%d case(s) did not serialise under %s/%s\n\n' \
        "$failed" "$encoding" "$endian"
    total=$((total + failed))
done

printf '%d case(s) did not serialise, over all modes run\n' "$total"
