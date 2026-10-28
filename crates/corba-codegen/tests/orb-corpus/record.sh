#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Run every row of `rows/` through the three ORB IDL compilers and record
# what each one generated, so `tests/orb_agreement.rs` can check the
# tables in `src/special_types.rs` and `src/stub.rs` against them.
#
#   . tools/interop-env.sh
#   export TAO_IDL="$TAO_ROOT/TAO_IDL/tao_idl"
#   crates/corba-codegen/tests/orb-corpus/record.sh crates/corba-codegen/tests/orb-corpus/recorded
#
# The compilers are on the measurement host and not on a laptop, which is
# why the output is recorded here and committed: the test reads the
# recording. A recording is not an oracle by itself — the ORB that made
# it is — so each file carries the compiler's own log beside its output.
#
# **One IDL file per row.** The first version ran the thirteen rows as
# one file and lost a whole ORB to one refusal: omniidl's C++ backend
# stops at `native`, tao_idl at `fixed`, and JacORB at a `CORBA::TypeCode`
# it will not look up. Each took the other twelve rows down with it. A
# refusal is a measurement — it is what that ORB says about that row —
# and it must not silence the rest.
#
# What is recorded is the declaration lines naming the probe operations,
# verbatim, and the compiler's log. The whole generated header is a
# thousand lines of ORB-internal scaffolding per row; re-run this script
# to see it.

set -eu

out="${1:?usage: record.sh <output directory>}"
here="$(cd "$(dirname "$0")" && pwd)"

mkdir -p "$out"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

tao_idl="${TAO_IDL:-${TAO_ROOT:-}/TAO_IDL/tao_idl}"
jacorb="${JACORB_HOME:-/opt/jacorb}"

# The operation names the corpus declares.
names='in_v|out_v|inout_v|ret_v|out_variable|out_fixed|ret_variable|ret_fixed'

banner() {
    printf '# %s\n' "$1"
    printf '# Recorded by crates/corba-codegen/tests/orb-corpus/record.sh.\n'
    printf '# Each block is one row of rows/, the compiler log, then every\n'
    printf '# generated line naming a probe operation, verbatim.\n'
}

: > "$work/omniorb.txt"
: > "$work/tao.txt"
: > "$work/jacorb.txt"

for idl in "$here"/rows/*.idl; do
    row="$(basename "$idl" .idl)"
    mkdir -p "$work/$row"
    cp "$idl" "$work/$row/row.idl"

    # --- omniORB ------------------------------------------------------
    {
        printf '\n=== %s\n' "$row"
        if command -v omniidl >/dev/null 2>&1; then
            (
                cd "$work/$row"
                omniidl -bcxx -Wbh=.hh -Wbs=SK.cc row.idl > omni.log 2>&1 || true
            )
            sed 's,^,log: ,' "$work/$row/omni.log"
            if [ -f "$work/$row/row.hh" ]; then
                grep -nE "$names" "$work/$row/row.hh" |
                    grep -E 'virtual' |
                    sed 's,^[0-9]*:[[:space:]]*,,'
            else
                printf 'log: (no header generated)\n'
            fi
        else
            printf 'log: omniidl is not on this host\n'
        fi
    } >> "$work/omniorb.txt"

    # --- TAO ----------------------------------------------------------
    {
        printf '\n=== %s\n' "$row"
        if [ -x "$tao_idl" ]; then
            (
                cd "$work/$row"
                "$tao_idl" -as -o . -I"${TAO_ROOT:-}" row.idl > tao.log 2>&1 || true
            )
            sort -u "$work/$row/tao.log" | sed 's,^,log: ,'
            if [ -f "$work/$row/rowC.h" ]; then
                # TAO writes the return type and name on one line and the
                # parameters on the next; both are kept.
                grep -A2 -nE "virtual .*($names) \(" "$work/$row/rowC.h" |
                    sed 's,^[0-9]*[-:][[:space:]]*,,' |
                    grep -vE '^(--|)$'
            else
                printf 'log: (no header generated)\n'
            fi
        else
            printf 'log: tao_idl is not on this host\n'
        fi
    } >> "$work/tao.txt"

    # --- JacORB -------------------------------------------------------
    {
        printf '\n=== %s\n' "$row"
        if [ -x "$jacorb/bin/idl" ]; then
            mkdir -p "$work/$row/java"
            (
                cd "$work/$row"
                "$jacorb/bin/idl" -d java row.idl > jacorb.log 2>&1 || true
            )
            grep -E 'SEVERE|giving up' "$work/$row/jacorb.log" | sort -u | sed 's,^,log: ,'
            operations="$work/$row/java/probe/SpecialOperations.java"
            if [ -f "$operations" ]; then
                grep -E "\b($names)\b" "$operations" | sed 's,^[[:space:]]*,,'
            else
                printf 'log: (no operations interface generated)\n'
            fi
        else
            printf 'log: jacorb is not on this host\n'
        fi
    } >> "$work/jacorb.txt"
done

{
    banner "omniidl — $(omniidl -V 2>&1 | head -1)"
    cat "$work/omniorb.txt"
} > "$out/omniorb.txt"
{
    banner "tao_idl — $tao_idl"
    cat "$work/tao.txt"
} > "$out/tao.txt"
{
    banner "jacorb idl — $jacorb"
    cat "$work/jacorb.txt"
} > "$out/jacorb.txt"

printf 'recorded %s rows into %s\n' "$(ls "$here"/rows/*.idl | wc -l)" "$out"
