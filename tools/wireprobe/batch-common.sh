# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The corpus as one translation unit per system, instead of one per case.
#
# ## Why
#
# Every case is its own IDL file, and a probe built per case runs the
# vendor's code generator and the compiler once per case: 335 cycles per
# system, roughly 3,700 across the tree, and hours of wall time. Nothing
# about a vendor requires that. `idlc`, `fastddsgen` and
# `opendds_idl` all take many types in one file and emit one type
# support for all of them, and a C or C++ program can hold every case's
# sample and every case's fill statements side by side.
#
# So the corpus becomes one IDL, one generator run, one compile, one
# link and one process that walks every case. Eleven builds where there
# were 3,700.
#
# ## The one thing that has to be arranged
#
# 307 of the 335 cases declare a top-level type called `Sample`, and 18
# distinct names cover all of them. Concatenating the files would be 307
# redefinitions. Each case is therefore wrapped in a module of its own —
# `module zd_c7 { … }` — which is a change to the type's *scoped name*
# and to nothing else.
#
# That it is nothing else is asserted and not assumed:
# `batch_verify_against` holds every byte the batched run produced
# against the per-case record that already exists. A module changes the
# generated identifier and the type's name; it does not reach member
# ids, extensibility, alignment or any framing decision, and the
# comparison is what says so rather than this paragraph.
#
# ## The three outcomes, inside a batch
#
# A case the front end refuses, or whose fill does not compile, must not
# take the batch down with it. Both failures name a file and a line, and
# `batch_blame` maps that line back to the case that owns it, so the
# failing case is excluded with the front end's or the compiler's own
# text and the run repeats without it. A case dropped that way is
# `refused` and carries the message; a case that never entered the batch
# because nothing could be constructed for it is a different word.
# Converging by exclusion and not by bisection: the error already says
# which line, and bisecting 120 refusals out of 335 would cost more
# builds than the arrangement saves.

# batch_wrap_idl <corpus dir> <case list file> <out idl> <out map>
#
# One IDL holding every case, each in its own module, and a map from
# `gen.idl` line number to case name so a diagnostic can be blamed.
batch_wrap_idl() {
    local corpus=$1 list=$2 out=$3 map=$4
    local index=0 case_name line
    : > "$out"
    : > "$map"
    while IFS= read -r case_name; do
        [ -n "$case_name" ] || continue
        index=$((index + 1))
        line=$(wc -l < "$out")
        printf '%s\t%s\t%s\n' "$((line + 1))" "$case_name" "zd_c$index" >> "$map"
        printf 'module zd_c%s {\n' "$index" >> "$out"
        cat "$corpus/$case_name/type.idl" >> "$out"
        printf '};\n' >> "$out"
    done < "$list"
}

# batch_scoped_type <corpus dir> <case> <module>
#
# The case's own top-level type, under the module the wrap gave it.
batch_scoped_type() {
    local corpus=$1 case_name=$2 module=$3 type_name
    type_name=$(cat "$corpus/$case_name/type" 2>/dev/null || echo Sample)
    printf '%s::%s' "$module" "$type_name"
}

# batch_blame <map> <line>
#
# Which case owns a line of `gen.idl`.
batch_blame() {
    local map=$1 line=$2
    awk -F'\t' -v want="$line" '$1 <= want { name = $2 } END { print name }' "$map"
}

# batch_blame_log <map> <log> <file> <last|all> [needle]
#
# Every case a diagnostic blames, one per line, with the message that
# named it. `<file>` is the generated file the line numbers refer to; a
# diagnostic about anything else is not a case's fault and is ignored
# here, so that a broken toolchain does not read as 335 refusals.
#
# `first` or `all`, because the two producers stop differently and the
# difference is the whole correctness of the loop:
#
#   * an **IDL front end** stops at the first error, so exactly one case
#     is to blame and it is the first one named;
#   * a **C or C++ compiler** reports every error it finds, so all of
#     them are blamed and one round removes the lot.
#
# `needle`, when given, is the substring a line must carry to count at
# all — `error:` for a compiler, so that a warning about one case cannot
# exclude it. `ignore`, when given, is an extended regular expression
# for lines that name a file and a line and are still not the fault of
# the case they point at.
#
# Both filters were paid for. `idlc` writes a warning per `@default`
# member and per unannotated aggregate, and blaming the first line that
# merely named `gen.idl` excluded the wrong case, left the real one in,
# and did it again next round: 76 rounds, 192 cases recorded as refused
# with a *warning* as the reason. Then it emits a cascade after the
# error it stopped on —
#
#     gen.idl:295:1: syntax error
#     gen.idl:396: fatal error: File write error
#
# — where line 295 is a `bitset` Cyclone has no form for and line 396 is
# a case that is perfectly fine. Blaming the last line instead cost 294
# wrongly excluded cases and left 112 of 1,340 cells carrying bytes
# where the per-case record has 872.
batch_blame_log() {
    local map=$1 log=$2 file=$3 which=${4:-all} needle=${5:-} ignore=${6:-}
    # Two diagnostic shapes, normalised to one. `idlc`, `fastddsgen`
    # and the C compilers write `gen.idl:295:1: …`; `tao_idl` writes
    # `Error - …: "gen.idl", line 4: …`. Reading only the first left
    # OpenDDS's front end unattributable, and an unattributable failure
    # stops the run — which is right, and was also 335 cases not
    # measured because of a comma.
    local normalised="$log.normalised"
    sed -e "s/\"$file\", line \([0-9]*\)/$file:\1/g" "$log" > "$normalised"
    log="$normalised"
    # Portable awk: `mawk` is what the test host has, and its `match`
    # takes no capture-group array. The line number is read by cutting
    # at the separator instead.
    awk -v map="$map" -v file="$file" -v which="$which" -v needle="$needle" \
        -v ignore="$ignore" '
        BEGIN {
            while ((getline line < map) > 0) {
                split(line, f, "\t")
                starts[++n] = f[1] + 0
                names[n] = f[2]
            }
        }
        {
            if (needle != "" && index($0, needle) == 0) next
            if (ignore != "" && $0 ~ ignore) next
            where = index($0, file)
            if (where == 0) next
            rest = substr($0, where + length(file))
            sep = substr(rest, 1, 1)
            if (sep != ":" && sep != "(") next
            rest = substr(rest, 2)
            digits = ""
            for (i = 1; i <= length(rest); i++) {
                c = substr(rest, i, 1)
                if (c < "0" || c > "9") break
                digits = digits c
            }
            if (digits == "") next
            want = digits + 0
            owner = ""
            for (i = 1; i <= n; i++) if (starts[i] <= want) owner = names[i]
            text = $0
            gsub(/\t/, " ", text)
            if (seen[owner]++) next
            print owner "\t" substr(text, 1, 200)
            if (which == "first") exit
        }
    ' "$log"
}
