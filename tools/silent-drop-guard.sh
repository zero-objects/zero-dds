#!/usr/bin/env bash
# Guard against silent drops in the rebuilt crates.
#
# A silent drop is a path that takes data or intent in, produces less —
# or something wrong — out, and reports success. Three of them were found
# by accident in one week; docs/silent-drop-sweep.md is the sweep that
# followed, and this is what keeps the next one from needing an accident.
#
# It greps for the eleven shapes that produce one and fails on any site
# that is not in tools/silent-drop-allowlist.tsv. The allowlist carries a
# reason per site, so adding one means writing down why it is not a
# defect — which is the whole mechanism. A guard that only counted would
# let a defect in as long as something else went out.
#
# Sites are keyed by shape, file and the text of the line, never by line
# number: a key that moved every time a function grew would be rewritten
# so often that nobody would read the diff.
#
#   bash tools/silent-drop-guard.sh                 check
#   bash tools/silent-drop-guard.sh --update        merge the scan in
#   bash tools/silent-drop-guard.sh --update --drop-stale
#                                                   ... and forget the
#                                                   rows the scan no
#                                                   longer finds
#
# `--update` merges rather than replaces. It rewrote the file wholesale
# once and took 21 rows another run had appended in the meantime with it,
# which is the very failure the guard exists to catch, in the guard's own
# tooling. Three rules now:
#
#   - a site the scan still finds keeps the reason it already had;
#   - a row the scan no longer finds is reported, never dropped in
#     silence;
#   - a run that would drop a row carrying a reason fails and writes
#     nothing. `--drop-stale` is how you say you read the report and mean
#     it.
#
# A lock directory serialises the read-modify-write, so two runs cannot
# interleave on the file at all.
#
# # Where this runs
#
# `crates/wire-conformance/tests/silent_drop_guard.rs` runs it under
# `cargo test`, which is the only chain a developer cannot skip and the
# reason that test exists. The guard was red for thirty commits without
# anyone acting on it, and the reason was not that it ran nowhere: the
# GitLab `silent-drop` job did run and did fail, in a pipeline where
# forty of sixty jobs were failing, so one more red job carried no
# signal. A check that only reports into a red pipeline is a check
# nobody reads.
#
# It also runs in scripts/pre-commit.sh and the `silent-drop` job of
# .gitlab-ci.yml, both calling this same script so the gate cannot
# drift between them. Two caveats, measured rather than assumed:
#
#   - scripts/pre-commit.sh is not installed as a git hook and nothing
#     in the tree installs it; running it is manual.
#   - .github/workflows/ci.yml is not the file GitHub runs. The root
#     repo's only remote is GitLab; the public mirror is the separate
#     `github/` checkout, whose own workflow has no silent-drop job.
set -euo pipefail

cd "$(dirname "$0")/.."
ALLOWLIST=tools/silent-drop-allowlist.tsv
LOCK=$ALLOWLIST.lock
LOCK_WAIT=120

# The rebuilt crates. A crate re-enters this list when it re-enters the
# workspace; the pre-demolition crates are deliberately absent.
CRATES=(
    crates/ast-core crates/ast crates/ast-layout
    crates/float128
    crates/idl-cst crates/idl-parse crates/idl-lower
    crates/lang-cst crates/lang-render crates/lang-rules
    crates/wire crates/wire-codec crates/wire-probe crates/wire-conformance
    # The DDS-XML front end. It lowers a type description into the same
    # model the IDL front end fills, so every shape below is a way for a
    # declaration to reach the wire without a member, a bound or a key.
    crates/xml-compose
)

# shape name -> extended regex. Each is a way for data to go missing.
SHAPES=(
    'unwrap_or|\.unwrap_or(_default|_else)?\('
    'ok_discard|\.ok\(\)'
    'let_else|let (Ok|Some|[A-Z][A-Za-z]*::[A-Z][A-Za-z]*)\(.*\) = .* else'
    'filter_map|\.(filter_map|flatten)\('
    'saturating|\.saturating_(add|sub|mul)\('
    'clamp|\.(min|max|clamp)\('
    'wildcard|_ => (\{ *\}|continue|None|\(\)|false|&\[\])'
    # Only the lints that mark a **value** being narrowed. It was every
    # `#[allow(`, and 35 of the 50 rows that produced were
    # `too_many_lines`, `too_many_arguments` and `many_single_char_names`
    # — lints about the shape of the code, which cannot drop an octet.
    # They all carried the same templated sentence in the allowlist,
    # which is what a reason column looks like when the shape asks a
    # question with no answer.
    'allow|#\[allow\(clippy::(cast_possible_truncation|cast_possible_wrap|cast_sign_loss|cast_precision_loss|cast_lossless)'
    # `try_from` as a bare word was removed: every way it loses a value
    # is already a hit under another shape — `try_from(..).unwrap_or(..)`
    # under `unwrap_or`, `try_from(..).ok()` under `ok_discard`, and
    # `let Ok(..) = try_from(..) else` under `let_else`. What it added
    # over those was `fn try_from(..)` impl headers and prose. It was
    # 59 rows and none of them were reachable only through it; dropping
    # it removes duplicates, not coverage.
    # The accessor family. members_of is a partial answer where
    # all_members_of is the whole one, and five defects in the sweep were
    # a caller that could not tell. operations_of is partial for the same
    # reason -- InterfaceNode carries bases, and all_operations_of is the
    # whole one -- and so is attributes_of, which has no whole
    # counterpart because only a test calls it. cases_of, enumerators_of
    # and bits_of are whole answers: Union, Enumeration and Bitmask carry
    # no base field. That is a reason a spec revision can retract, which
    # is why they are watched. bitfields_of is not in the pattern, so its
    # call sites are not watched; docs/silent-drop-sweep.md records that.
    'partial_walk|members_of|cases_of|enumerators_of|bits_of|attributes_of|operations_of'
    'empty_ok|return Ok\(\(\)\);|return Ok\(Vec::new\(\)\);|=> \{\}'
)

# Shapes watched in every crate, not only the front end's. A list or a
# buffer cut to the caller's capacity is a value going missing wherever it
# happens; seven C API calls did it and reported Ok until the C API's list
# contract (docs/specs/zerodds-c-api-1.0.md §2.8) gave the total back. A
# site in this shape names in its reason how the caller learns of the cut.
WIDE_SHAPES=(
    'capacity_clip|\.min\((cap|out_cap|capacity|max_len|buf_len|out_len)\)'
)

scan() {
    {
        scan_shapes "${SHAPES[@]}" -- "${CRATES[@]}"
        scan_shapes "${WIDE_SHAPES[@]}" -- crates tools
    } | sort | uniq -c | sed -e 's/^ *\([0-9][0-9]*\) /\1\t/' | sort
}

# scan_shapes SHAPE... -- ROOT...  (no nameref: macOS ships bash 3.2)
scan_shapes() {
    local shapes=()
    while [[ $# -gt 0 && $1 != -- ]]; do
        shapes+=("$1")
        shift
    done
    shift
    local roots=("$@")
    local entry name pattern
    for entry in "${shapes[@]}"; do
        name=${entry%%|*}
        pattern=${entry#*|}
        # A shape that matches nothing exits 1, which is not an error and
        # must not abort the scan half way; anything above 1 is grep
        # failing and does abort it, because a scan that skipped a shape
        # would clear every allowlist row belonging to it.
        { grep -rn --include='*.rs' -E "$pattern" "${roots[@]}" 2>/dev/null || [ $? = 1 ]; } \
            | { grep '/src/' || [ $? = 1 ]; } \
            | while IFS= read -r hit; do
                # path:line:code -> shape \t path \t trimmed code
                local file code
                file=${hit%%:*}
                code=${hit#*:}
                code=${code#*:}
                code=$(printf '%s' "$code" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
                # A comment cannot drop a value. Skipped because the
                # shapes are word patterns and prose uses the same
                # words: the sentence explaining why an
                # `.unwrap_or(declared)` was removed matches the
                # `unwrap_or` shape, so the guard fired on the
                # documentation of its own fix and the cheapest way out
                # was to describe the defect less precisely. A guard
                # that charges for writing down the reason is a guard
                # that gets reasons like "checked".
                #
                # `//` only. A block-comment continuation line starts
                # with `*` by convention, and so does a Rust deref
                # statement -- `*slot = read().unwrap_or(0);` is a drop
                # that a `*` pattern would hide. Measured before
                # narrowing it: the guarded crates hold no `/*` block
                # comment and no line beginning with `*`, so the two
                # patterns covered nothing and only left the hole.
                case $code in
                    '//'*) continue ;;
                esac
                printf '%s\t%s\t%s\n' "$name" "$file" "$code"
            done
    done
}

current=$(mktemp)
expected=$(mktemp)
body=$(mktemp)
stale=$(mktemp)
lock_held=0
cleanup() {
    rm -f "$current" "$expected" "$body" "$stale" "$ALLOWLIST.tmp"
    [[ $lock_held == 1 ]] && rmdir "$LOCK" 2>/dev/null
    return 0
}
trap cleanup EXIT

# The header the file carries. Written here rather than preserved from
# the old file, so a hand edit to the prose does not survive as the only
# copy of it.
header() {
    echo "# Sites the silent-drop guard knows about, with the reason each is"
    echo "# not a defect. Merged by tools/silent-drop-guard.sh --update;"
    echo "# the reason column is written by hand and must survive a rewrite."
    echo "# Columns: count, shape, file, code, reason."
    echo "#"
    echo "# Adding a row means writing down why the site does not drop data."
    echo "# 'the caller checks' is only a reason if you can name the caller."
    echo "#"
    echo "# --update never deletes a row carrying a reason. If the scan stops"
    echo "# finding one it says so and refuses; --drop-stale is the answer."
}

if [[ ${1:-} == --update ]]; then
    drop_stale=no
    case "${2:-}" in
        "") ;;
        --drop-stale) drop_stale=yes ;;
        *) echo "silent-drop-guard: \`$2\` is not an option" >&2; exit 2 ;;
    esac

    # Serialise the read-modify-write. Several agents share this file and
    # a rewrite that read it before another's append would drop that
    # append -- which is what happened, and is why the merge below exists.
    waited=0
    until mkdir "$LOCK" 2>/dev/null; do
        if (( waited >= LOCK_WAIT )); then
            echo "silent-drop-guard: $LOCK has been held for ${waited}s." >&2
            echo "Remove it if no other run is in flight." >&2
            exit 1
        fi
        sleep 1
        waited=$((waited + 1))
    done
    lock_held=1

    # Scan under the lock, not before it: a scan taken outside would be
    # merged against an allowlist that had moved on since.
    scan > "$current"

    # An absent allowlist is an empty one, so the first run takes the same
    # path as every later one and its rows come out UNREVIEWED like any
    # other new site.
    previous=$ALLOWLIST
    [[ -f $ALLOWLIST ]] || previous=/dev/null

    # Pass one reads the allowlist, pass two the scan. A site in both
    # keeps its reason; a site only the scan has is UNREVIEWED; a row only
    # the allowlist has goes to the stale report and never to the body.
    merge=0
    # The passes are told apart by FILENAME and not by NR==FNR: the first
    # file is /dev/null on a first run, and an empty first file makes
    # NR==FNR true for the second file's first record — which would read
    # one scanned site as an allowlist row and lose it.
    awk -F'\t' -v body="$body" -v stale="$stale" -v previous="$previous" '
        FILENAME == previous {
            if ($0 ~ /^#/ || $0 ~ /^[[:space:]]*$/) next
            key = $2 FS $3 FS $4
            reason[key] = (NF >= 5 ? $5 : "")
            held_count[key] = $1
            line[key] = $0
            order[++held] = key
            next
        }
        {
            key = $2 FS $3 FS $4
            seen[key] = 1
            # The count is deliberately not part of the key: a site that
            # gained an instance is the same site and must not lose the
            # judgement wholesale. It is part of the *carry* decision,
            # though, and that asymmetry is the point.
            #
            # A row that grew has instances nobody read. Carrying the
            # reason onto them would launder them under a sentence
            # written about the ones already there — the guard doing, in
            # its own merge, what it exists to catch. `_ => None,` in
            # crates/ast-layout/src/key.rs went from 11 to 18 that way.
            # A row that shrank or held still has no unread instance, so
            # the reason still covers every one of them and carries.
            grew = (key in reason && $1 + 0 > held_count[key] + 0)
            carried = (key in reason && reason[key] != "" && !grew) \
                ? reason[key] : "UNREVIEWED"
            if (grew && reason[key] != "" && reason[key] != "UNREVIEWED") {
                printf "  GREW      %s -> %s instances\t%s\n", \
                    held_count[key], $1, line[key] > stale
            }
            print $0 "\t" carried > body
        }
        END {
            for (i = 1; i <= held; i++) {
                key = order[i]
                if (key in seen) continue
                if (reason[key] != "" && reason[key] != "UNREVIEWED") {
                    printf "  REASONED  %s\n", line[key] > stale
                    reasoned++
                } else {
                    printf "  UNJUDGED  %s\n", line[key] > stale
                }
            }
            exit (reasoned > 0) ? 1 : 0
        }
    ' "$previous" "$current" || merge=$?

    if [[ -s $stale ]]; then
        echo "silent-drop-guard: rows whose site changed under them:" >&2
        echo "  REASONED/UNJUDGED — the scan no longer finds the row." >&2
        echo "  GREW              — the site gained instances nobody read;" >&2
        echo "                      the reason was dropped, not carried." >&2
        cat "$stale" >&2
        echo >&2
    fi

    if [[ $merge != 0 && $drop_stale != yes ]]; then
        cat >&2 <<'MSG'
A REASONED row is one somebody judged. Losing it loses the judgement, so
nothing was written. Either the site moved and the scan will find it again
once the tree is consistent, or it is genuinely gone -- and then:

    bash tools/silent-drop-guard.sh --update --drop-stale
MSG
        exit 1
    fi

    { header; cat "$body"; } > "$ALLOWLIST.tmp"
    mv "$ALLOWLIST.tmp" "$ALLOWLIST"
    echo "wrote $ALLOWLIST ($(wc -l < "$body" | tr -d ' ') site(s))"
    exit 0
fi

scan > "$current"

if [[ ! -f $ALLOWLIST ]]; then
    echo "silent-drop-guard: $ALLOWLIST is missing; run --update" >&2
    exit 1
fi

grep -v '^#' "$ALLOWLIST" | cut -f1-4 | sed '/^[[:space:]]*$/d' | sort > "$expected"

if diff_out=$(diff "$expected" "$current"); then
    # A row without a reason is a site nobody judged. The allowlist
    # exists for the reasons, not for the count.
    if ! awk -F'\t' '/^#/ || /^[[:space:]]*$/ { next }
                     NF < 5 || $5 == "" || $5 == "UNREVIEWED" { print NR ": " $0; bad=1 }
                     END { exit bad ? 1 : 0 }' "$ALLOWLIST" >&2; then
        echo "silent-drop-guard: the rows above carry no reason." >&2
        exit 1
    fi
    echo "silent-drop-guard: $(wc -l < "$current" | tr -d ' ') site(s), all accounted for."
    exit 0
fi

echo "silent-drop-guard: the sites do not match the allowlist." >&2
echo >&2
echo "$diff_out" | sed -n 's/^> /  NEW      /p' >&2
echo "$diff_out" | sed -n 's/^< /  GONE     /p' >&2
cat >&2 <<'MSG'

A NEW line is a site the sweep has not judged. Read the function it sits
in and decide whether data can go missing there. If it can, fix it —
handle the case, or return an error that names the construct. If it
cannot, run:

    bash tools/silent-drop-guard.sh --update

and replace the UNREVIEWED reason with why, in one line. A GONE line is a
row the scan no longer finds; --update reports it and, if it carries a
reason, refuses to drop it until you say --drop-stale.

docs/silent-drop-sweep.md has the eleven defects this found and the
shapes it cannot reach.
MSG
exit 1
