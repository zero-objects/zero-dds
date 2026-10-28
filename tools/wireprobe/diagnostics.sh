# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Keep what a probe's toolchain said, including when the probe succeeded.
#
# ## Why
#
# `tools/interop-results/warning-sweep.sh` runs each system's **front
# end** and stops, which is what makes it cheap and is also its limit: a
# probe then compiles, and until this file existed every probe wrote its
# compiler's output to a `cc.log` inside a `mktemp -d`, read back
# `error:` lines only when the build had failed, and deleted the rest
# with the directory.
#
# The cost is measured. The `wchar` fill defect was a multi-character
# constant — `sample.wide_letter = '\u{e4}';` emitted as a narrow C
# character literal, which gcc warns about by default under
# `-Wmultichar` and compiles to `c3a4`. That warning was printed on
# every probe build carrying the literal. It was deleted every time, and
# the `c3a4` it explained stood in the results as Cyclone's and RTI's
# own `wchar` octets — 56 rows, two vendors recorded as diverging — until
# somebody re-derived it from the bytes.
#
# ## What it records
#
# One row per diagnostic line, in the shape
# `tools/interop-results/sources/warnings.tsv` already has, so a
# compiler is one more `program` beside `idlc` and `tao_idl`:
#
#   <case>  <system>  <program>  <exit>  <severity>  <text>
#
# `severity` is the word the line itself carries and never the one the
# exit status implies — the same rule `warning-sweep.sh` gives, and for
# the same measured reason: `tao_idl` prints `Error - …` and exits 0.
#
# ## Where it writes
#
# `WIREPROBE_DIAGNOSTICS` names a **directory**, not a file. A sweep runs
# `--jobs` probes at once and two appends to one file interleave, so each
# invocation writes a file of its own and the sweep concatenates them.
# Unset, every function here returns without doing anything, so a
# single-shot probe run behaves as it did.
#
# ## The cache
#
# A probe binary is kept and reused, so the second sweep over a case does
# not compile and has nothing to say. That would lose every warning on a
# re-record, which is the same defect one level down. So the log is
# stored in the cache entry beside the binary and replayed on a hit:
# `probe_diagnostics_publish` puts it there, `probe_diagnostics_cached`
# reads it back.

# probe_diagnostics_case <idl path>
#
# The corpus names a case by its directory, everywhere else in this tree.
probe_diagnostics_case() {
    basename "$(dirname "${1:-}")"
}

# probe_diagnostics_clean <text>
#
# What is a property of the run rather than of the case, removed: the
# temporary directory the toolchain was reading in, the line and column
# numbers that depend on which other cases shared a chunk, and the
# escape sequences a colouring compiler emits for the terminal it
# thought it had. Nothing else. Same rules as `wireprobe-batch`'s
# `refuse`, which normalises for the same reason.
probe_diagnostics_clean() {
    printf '%s' "$1" | tr '\t' ' ' \
        | sed -e "s/$(printf '\033')\\[[0-9;]*m//g" \
              -e 's|/tmp/[A-Za-z0-9._-]*/||g' \
              -e 's|:[0-9][0-9]*:[0-9][0-9]*:|:|g' \
              -e 's|:[0-9][0-9]*:|:|g' \
        | cut -c1-400 | iconv -f utf-8 -t utf-8 -c
}

# probe_diagnostics_is_context <line>
#
# Whether the line is a compiler's *context* rather than a diagnostic.
# gcc and g++ print the source line under the message, a caret under
# that, and an include chain above it:
#
#        4 | sample.bounded = (char *)"ok";
#          |                ^
#     In file included from genPubSubTypes.cxx:
#                      from /opt/fastdds/include/…/Locator.hpp:22,
#
# None of them is a statement about a case, and recorded as rows they
# outnumbered the diagnostics four to one on the first measured sweep.
# The shapes are listed rather than inferred, so a line that is *not*
# one of them is kept even when it carries no severity word — which is
# what `unclassified` is for.
probe_diagnostics_is_context() {
    case "$1" in
        *"In file included from"*)          return 0 ;;
        # gcc's location headers, printed above the diagnostic they
        # scope and ending in a colon. They name the function a message
        # is about and are not messages.
        *"In function "*:|*"In member function "*:|*"In lambda function"*:) return 0 ;;
        *"At global scope"*:|*"In instantiation of "*:)                     return 0 ;;
        *"In constructor "*:|*"In destructor "*:|*"In static member function "*:) return 0 ;;
    esac
    printf '%s' "$1" | grep -qE '^[[:space:]]*[0-9]+[[:space:]]*\|' && return 0
    printf '%s' "$1" | grep -qE '^[[:space:]]*\|' && return 0
    printf '%s' "$1" | grep -qE '^[[:space:]]+from [^[:space:]]' && return 0
    return 1
}

# probe_diagnostics_severity <line>
#
# The word the diagnostic claims. `unclassified` is deliberate and is
# never a silent drop: a toolchain that starts saying something in a
# shape nothing here knows is exactly what this file exists to notice.
probe_diagnostics_severity() {
    case "$1" in
        *[Ww]arning:*|*"[-W"*)         printf 'warning'; return ;;
        *[Ww]arning*)                  printf 'warning'; return ;;
        *[Ee]rror:*|*[Ee]rror\ -*)     printf 'error';   return ;;
        *[Ee]rror*)                    printf 'error';   return ;;
        *note:*|*Note:*)               printf 'note';    return ;;
    esac
    # No severity word and no switch. A compiler's diagnostics always
    # carry one of the three, so what is left is a front end's progress
    # output — `Processing the file gen.idl...`, `openjdk version …`,
    # `Adding project: gen.idl`. Those lines are a front end's, and the
    # authority on a front end's lines is `warning-sweep.sh`, which runs
    # the same program over the same file and reads it with that
    # system's own severity table. Recording them again here as
    # `unclassified` would put a generator's banner in the list that
    # exists to show a front end saying something new.
    #
    # `noise` and not a drop: the row is in the file, and a line that
    # does carry a severity word or a switch and has no entry is still
    # `unclassified` and still loud.
    printf 'noise'
}

# probe_diagnostics_rows <system> <program> <exit> <log> <idl> <out>
#
# Every line of <log>, as rows, appended to <out>. Called after a
# toolchain runs, whether it succeeded or not — a clean build that
# warned is the case this exists for, and it is the case the old
# `grep error: on failure` could not reach.
probe_diagnostics_rows() {
    [ -f "${4:-}" ] || return 0
    local case_name line severity text
    case_name="$(probe_diagnostics_case "${5:-}")"
    while IFS= read -r line; do
        [ -n "$line" ] || continue
        probe_diagnostics_is_context "$line" && continue
        severity="$(probe_diagnostics_severity "$line")"
        text="$(probe_diagnostics_clean "$line")"
        [ -n "$text" ] || continue
        printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$case_name" "$1" "$2" "$3" "$severity" "$text" >> "$6"
    done < "$4"
    return 0
}

# probe_diagnostics_record <system> <program> <exit> <log> <idl>
#
# The same, into a file of this run's own under `WIREPROBE_DIAGNOSTICS`.
probe_diagnostics_record() {
    [ -n "${WIREPROBE_DIAGNOSTICS:-}" ] || return 0
    [ -f "${4:-}" ] || return 0
    mkdir -p "$WIREPROBE_DIAGNOSTICS" || return 0
    probe_diagnostics_rows "$1" "$2" "$3" "$4" "${5:-}" \
        "$WIREPROBE_DIAGNOSTICS/$$.$(date +%s%N 2>/dev/null || date +%s).$RANDOM.tsv"
}

# probe_diagnostics_publish <work dir> <cache build dir>
#
# Keep the build's **rows** with the binary, not its raw log. A row
# already carries the program that printed it — `idlc`, `cc`, `javac` —
# and a raw log replayed later would have to be attributed again, under
# one name for all of it. Measured: it came back as a program called
# `toolchain`, which is no program, and the same diagnostic then
# appeared twice under two names.
probe_diagnostics_publish() {
    [ -n "${2:-}" ] || return 0
    [ -f "${1:-}/toolchain.rows.tsv" ] || return 0
    cp "$1/toolchain.rows.tsv" "$2/toolchain.rows.tsv" 2>/dev/null || true
    return 0
}

# probe_diagnostics_cached <system> <cache dir> <idl>
#
# Replay what the build said, on a run that did not build. Without this
# a re-record is silent for every case it did not recompile, which is
# the same loss the temporary directory used to cause. The rows are
# copied as they were recorded, so a cache hit and a cold build put the
# identical row in the file.
probe_diagnostics_cached() {
    [ -n "${WIREPROBE_DIAGNOSTICS:-}" ] || return 0
    [ -n "${2:-}" ] || return 0
    mkdir -p "$WIREPROBE_DIAGNOSTICS" || return 0
    if [ ! -f "$2/toolchain.rows.tsv" ]; then
        # A hit on an entry with nothing stored. `probe_cache_key`
        # carries the mechanism's version so this cannot happen for an
        # entry written since, which makes it worth counting rather
        # than passing over: it reports nothing, and reporting nothing
        # is what a build with nothing to say does.
        printf '%s\n' "$2" >> "$WIREPROBE_DIAGNOSTICS/.silent-hits"
        return 0
    fi
    cp "$2/toolchain.rows.tsv" \
       "$WIREPROBE_DIAGNOSTICS/$$.$(date +%s%N 2>/dev/null || date +%s).$RANDOM.tsv" \
       2>/dev/null || true
    return 0
}

# probe_diagnostics_after <system> <work dir> <idl> <exit>
#
# Every log the build left behind, recorded, and concatenated into one
# `toolchain.log` for the cache to carry. Called immediately after the
# build subshell returns — on success as well as on failure, which is
# the whole difference from what was there before.
#
# The program is the log's name, because that is what each probe calls
# its steps: `idlc.log` is Cyclone's front end, `gen.log` RTI's and Fast
# DDS's, `idl.log`/`tao.log` the ORBs' and OpenDDS's two, `cc.log` the C
# or C++ compiler and `javac.log` the Java one. A log this list does not
# know is still recorded, under its own stem, rather than skipped.
probe_diagnostics_after() {
    local system="$1" work="$2" idl="$3" status="${4:-0}" log stem program rows
    rows="$work/toolchain.rows.tsv"
    : > "$rows" 2>/dev/null || return 0
    for log in "$work"/*.log; do
        [ -f "$log" ] || continue
        stem="$(basename "$log" .log)"
        [ "$stem" = "toolchain" ] && continue
        # The log's stem is what the probe called the step, and `gen`
        # and `idl` are files rather than programs. The system knows
        # which generator wrote them.
        case "$stem:$system" in
            cc:*)         program=cc ;;
            javac:*)      program=javac ;;
            idlc:*)       program=idlc ;;
            gen:rti)      program=rtiddsgen ;;
            gen:fastdds)  program=fastddsgen ;;
            idl:omniorb)  program=omniidl ;;
            idl:tao)      program=tao_idl ;;
            idl:opendds)  program=opendds_idl ;;
            idl:jacorb)   program=idlj ;;
            gen:hdds)     program=hddsgen ;;
            build:hdds)   program=cargo ;;
            tao:*)        program=tao_idl ;;
            *)            program="$stem" ;;
        esac
        probe_diagnostics_rows "$system" "$program" "$status" "$log" "$idl" "$rows"
    done
    if [ -n "${WIREPROBE_DIAGNOSTICS:-}" ] && [ -s "$rows" ]; then
        mkdir -p "$WIREPROBE_DIAGNOSTICS" || return 0
        cp "$rows" \
           "$WIREPROBE_DIAGNOSTICS/$$.$(date +%s%N 2>/dev/null || date +%s).$RANDOM.tsv" \
           2>/dev/null || true
    fi
    return 0
}

# probe_diagnostics_merge <dir> <dest> <system...>
#
# The rows this run produced, into `<dest>`, keeping every row the run
# did not measure.
#
# The same rule `vendor-corpus-sweep.sh` follows for its own file, and
# for the same reason: a sweep that wrote only what it measured would
# delete every system it was not asked about, and the tree would still
# verify because the sources and the tree would have shrunk together.
#
# `PROBE_DIAGNOSTICS_CASES`, a file of one case name per line, narrows
# "measured" from the system to the (system, case) pair. Only
# `vendor-corpus-sweep.sh` has a `--cases`, and it sets this when it was
# given one. **Without it a subset run deleted every diagnostic of the
# swept systems for the cases it never touched**: measured on a
# `--vendors fastdds --cases @25-cases` re-record, which took the file
# from 107 rows to 16 and reported success. That is the same defect this
# function exists to prevent, one key narrower than the key it used —
# and the byte rows two files over already keyed on the pair.
#
# This is a file of its own and not part of `sources/warnings.tsv`
# because `warning-sweep.sh` writes that one whole — it `mv`s its output
# over it — so a compiler row put there would survive exactly until the
# next front-end sweep. One writer per file is the invariant `sources/`
# already has.
probe_diagnostics_merge() {
    local dir="$1" dest="$2"
    shift 2
    local kept new
    kept="$(mktemp)"; new="$(mktemp)"
    if [ -d "$dir" ]; then
        cat "$dir"/*.tsv 2>/dev/null | LC_ALL=C sort -u > "$new"
    fi
    if [ -f "$dest" ]; then
        local pattern=""
        for s in "$@"; do pattern="${pattern}${pattern:+|}$s"; done
        if [ -n "${PROBE_DIAGNOSTICS_CASES:-}" ] && [ -f "${PROBE_DIAGNOSTICS_CASES}" ]; then
            # A row survives unless this run measured its system *and*
            # its case. A row filed under `-` names no case, so it is
            # never one this run replaced, and it survives.
            awk -F'\t' -v p="^($pattern)$" '
                NR == FNR { swept[$0] = 1; next }
                !($2 ~ p && ($1 in swept))
            ' "$PROBE_DIAGNOSTICS_CASES" "$dest" > "$kept"
        else
            awk -F'\t' -v p="^($pattern)$" '$2 !~ p' "$dest" > "$kept"
        fi
    fi
    local added
    added=$(wc -l < "$new")
    LC_ALL=C sort -u "$kept" "$new" > "$dest.new" && mv "$dest.new" "$dest"
    rm -f "$kept" "$new"
    local orphans silent
    orphans=$(awk -F'\t' '$1 == "-"' "$dest" | wc -l)
    silent=$(sort -u "$dir/.silent-hits" 2>/dev/null | grep -c . || true)
    echo "toolchain diagnostics: $added new row(s); $(wc -l < "$dest") in $dest" >&2
    # A row the batch could not pin to a case is in the file and is said
    # out loud, because it is the one kind that reaches no row of the
    # matrix and would otherwise be a number nobody sees.
    [ "$orphans" -gt 0 ] && echo "toolchain diagnostics: $orphans row(s) name no case; \
they are in the file under \`-\` and belong to a chunk rather than a case" >&2
    # A hit that carried nothing is not a build that said nothing, and
    # the two look the same in the row count.
    [ "${silent:-0}" -gt 0 ] && echo "toolchain diagnostics: $silent cache entr(ies) held no \
recorded diagnostics; they were written before the mechanism and this run reported nothing for \
them" >&2
    return 0
}

# probe_diagnostics_batch <system> <build dir> <map> <log> <program> <exit>
#
# The batched path. `wireprobe-batch` compiles one chunk holding many
# cases, so a diagnostic has to be attributed before it can be a row —
# which is what the script already does to decide which case to
# exclude, by the same three shapes:
#
#   gen.idl:<n>:   the line, through the wrap's map
#   zd_c<i>        the module the wrap gave one case
#   fill_<i>.inc, zd_case_<i>   the driver's function for one case
#
# A line matching none of them is recorded under the case `-` rather
# than dropped. An unattributable diagnostic is a real thing — Fast
# DDS's generator prints a stack trace with no line number in it — and
# the file saying so is better than the file not having it.
probe_diagnostics_batch() {
    [ -n "${WIREPROBE_DIAGNOSTICS:-}" ] || return 0
    local system="$1" build="$2" map="$3" log="$4" program="$5" status="${6:-0}"
    [ -f "$log" ] || return 0
    [ -f "$map" ] || return 0
    mkdir -p "$WIREPROBE_DIAGNOSTICS" || return 0
    local out line severity text case_name n
    out="$WIREPROBE_DIAGNOSTICS/$$.$(date +%s%N 2>/dev/null || date +%s).$RANDOM.tsv"
    local only
    only="$(awk -F'\t' '{print $2}' "$map" | sort -u)"
    [ "$(printf '%s\n' "$only" | grep -c .)" -eq 1 ] || only=""
    while IFS= read -r line; do
        [ -n "$line" ] || continue
        probe_diagnostics_is_context "$line" && continue
        # A chunk that has been split down to one case has one case in
        # its map, and every line of that build is that case's. Exact,
        # not a guess — it is why the batch splits at all.
        case_name="$only"
        if [ -z "$case_name" ]; then
            n="$(printf '%s' "$line" | sed -n 's|.*gen\.idl:\([0-9][0-9]*\):.*|\1|p' | head -1)"
            [ -z "$n" ] && n="$(printf '%s' "$line" \
                | sed -n 's|.*"gen\.idl", line \([0-9][0-9]*\):.*|\1|p' | head -1)"
            if [ -n "$n" ]; then
                case_name="$(batch_blame "$map" "$n" 2>/dev/null || true)"
            fi
            if [ -z "$case_name" ]; then
                n="$(printf '%s' "$line" | grep -oE 'zd_c[0-9]+' | head -1)"
                [ -n "$n" ] && case_name="$(awk -F'\t' -v m="$n" '$3 == m {print $2}' "$map" | head -1)"
            fi
            if [ -z "$case_name" ]; then
                n="$(printf '%s' "$line" | grep -oE 'fill_[0-9]+\.inc|zd_case_[0-9]+' \
                     | head -1 | grep -oE '[0-9]+')"
                [ -n "$n" ] && case_name="$(awk -F'\t' -v k="$n" 'NR == k {print $2}' "$map")"
            fi
            # `-` is a line the chunk could not attribute, and it is
            # kept: `fastddsgen` prints a stack trace with no line
            # number in it, and a file that says so is better than a
            # file that does not have it.
            [ -n "$case_name" ] || case_name="-"
        fi
        severity="$(probe_diagnostics_severity "$line")"
        text="$(probe_diagnostics_clean "$line")"
        [ -n "$text" ] || continue
        printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$case_name" "$system" "$program" "$status" "$severity" "$text" >> "$out"
    done < "$log"
    return 0
}

# -- the generated sources ------------------------------------------------
#
# What a front end produced, kept where a reader can see it.
#
# ## Why
#
# The same reason as the rows above, one layer out. Every probe runs a
# vendor's generator over the corpus — `idlc`, `fastddsgen`,
# `opendds_idl`, `tao_idl`, `omniidl`, `jacorb` — and every probe then
# deletes what it produced with its `mktemp -d`. The comparison a reader
# most wants is the one that exists for a moment on every measured run
# and is never written down: **what does each toolchain make of the same
# IDL**.
#
# It is not a byte question, so no octet ledger holds it.
# `docs/structural-review.md` had to reconstruct it by hand to find that
# five columns flatten an inherited struct and HDDS nests it, and that
# two columns discard the IDL spelling of a member a keyword forced to
# move — which §7.3.1.2.1.1 needs to compute a hashed member id.
#
# ## How
#
# Off by default, because a corpus run is 376 cases and the sources are
# larger than the octets. `WIREPROBE_KEEP_GENERATED` names a directory
# and every probe that calls this fills
# `<dir>/<case>/<system>/` with what its front end wrote.

# probe_keep_generated <work dir> <system> <idl path> [pattern ...]
#
# Copies the generated files out of the work directory before its trap
# removes it. A pattern selects what the front end produced rather than
# what the probe wrote around it; with none given, everything but the
# probe's own scaffolding is kept.
#
# Silent when the variable is unset, which is the normal run. Never
# fails the probe: a measurement must not be lost because a copy for a
# reader did not work.
probe_keep_generated() {
    local work="${1:-}" system="${2:-}" idl="${3:-}"
    shift 3 2>/dev/null || true
    [ -n "${WIREPROBE_KEEP_GENERATED:-}" ] || return 0
    [ -d "$work" ] || return 0
    local case_name
    case_name="$(probe_diagnostics_case "$idl")"
    local out="$WIREPROBE_KEEP_GENERATED/$case_name/$system"
    mkdir -p "$out" 2>/dev/null || return 0
    if [ "$#" -gt 0 ]; then
        local pattern
        for pattern in "$@"; do
            # shellcheck disable=SC2086
            cp $work/$pattern "$out/" 2>/dev/null || true
        done
    else
        # What the **front end** wrote, which is not everything in the
        # work directory. A vendor probe also writes its own harness —
        # `probe.c` from a template with one name substituted, the
        # `fill.inc` our filler emits, a `names.sh`, the diagnostic
        # rows — and none of that came out of the toolchain. Keeping it
        # would put a page that says "what each toolchain made of this
        # IDL" three-quarters in the wrong, and it is most of the bulk:
        # for `primitive-mixed` the harness files are 28 of 208 KiB
        # before the object files.
        #
        # A column whose generator *does* write the probe passes an
        # explicit pattern instead; ours do.
        find "$work" -maxdepth 1 -type f \
            ! -name '*.o' ! -name '*.class' ! -name '*.jar' \
            ! -name 'probe' ! -name '*.bin' ! -name '*.log' \
            ! -name 'probe.*' ! -name '*.inc' ! -name '*.inc.keep' \
            ! -name '*.tsv' ! -name '*.sh' ! -name 'root' \
            -exec cp {} "$out/" \; 2>/dev/null || true
    fi
    return 0
}

# probe_generated_publish <work dir> <cache build dir>
#
# The generated sources, kept with the binary the build produced.
#
# A probe that hits the build cache never runs its front end — the
# whole point of the cache — so the work directory of a cached run
# holds the IDL that went in and nothing that came out. The rows above
# have this same shape and solve it the same way: publish once at build
# time, replay on every hit.
probe_generated_publish() {
    local work="${1:-}" build="${2:-}"
    [ -d "$work" ] && [ -d "$build" ] || return 0
    mkdir -p "$build/generated" 2>/dev/null || return 0
    find "$work" -maxdepth 1 -type f \
        ! -name '*.o' ! -name '*.class' ! -name '*.jar' \
        ! -name 'probe' ! -name '*.bin' ! -name '*.log' ! -name '*.keep' \
        -exec cp {} "$build/generated/" \; 2>/dev/null || true
    return 0
}

# probe_generated_cached <cache dir> <work dir>
#
# The published sources, back into the work directory, so the exit trap
# copies the same files out of a cached run as out of a built one.
probe_generated_cached() {
    local cached="${1:-}" work="${2:-}"
    [ -n "${WIREPROBE_KEEP_GENERATED:-}" ] || return 0
    [ -d "$cached/generated" ] && [ -d "$work" ] || return 0
    cp "$cached/generated/"* "$work/" 2>/dev/null || true
    return 0
}
