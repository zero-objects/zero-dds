#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# fixed-bytes.sh — `fixed<P,S>` through ACE's CDR stream, which is the
# encoder OpenDDS's generated `write_fixed` calls.
#
#   tools/unspecified-reach/fixed-bytes.sh
#   tools/unspecified-reach/fixed-bytes.sh --out fixed.tsv
#
#   --work <dir>   where the probe is compiled
#   --out <file>   the report, as TSV; default stdout
#   --require      ACE absent fails the run
#
# One row per value:
#
#   declared  value  bytes  length
#
# ## Why this exists when tao_idl refuses the keyword
#
# `tao_idl` rejects `fixed` with "fixed data types are not supported",
# so the byte gate cannot reach the type through OpenDDS's IDL. But
# `opendds_idl` accepts it and generates
# `write_fixed(value.f.to_ace_fixed())`, so OpenDDS's encoder *is*
# `ACE_CDR::Fixed`, and `ACE_CDR::Fixed` takes a value from a string
# without any front end at all. That is the whole trick: a third
# ORB-family witness for a rule XTypes does not carry, reached around
# the front end that refuses it.
#
# The rule is CORBA 3.3 part 2 §9.3.2.8 — packed BCD, two digits an
# octet, most significant first, the low nibble of the last octet the
# sign. `ACE_CDR::Fixed` carries the digit count of the value it parsed
# rather than of the type that declared it, exactly as JacORB's
# `BigDecimal` does, so it and this project part company on a value with
# fewer digits than its type. That is a property of a runtime type with
# no declared precision and not a second reading of the clause; row P4
# of `docs/wire-feature-inventory.md` has the argument for why the
# declared count is the half a reader can use.
#
# Recorded as `crates/ast-layout/tests/fixtures/unspecified_constructs.md` §3.
set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../interop-vendors.sh
. "$HERE/../interop-vendors.sh"

WORK=""
OUT=""
REQUIRE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --work)    WORK="$2"; shift 2 ;;
        --out)     OUT="$2"; shift 2 ;;
        --require) REQUIRE=1; shift ;;
        *) echo "fixed-bytes.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done

emit() { if [ -n "$OUT" ]; then printf '%s\n' "$*" >> "$OUT"; else printf '%s\n' "$*"; fi; }
[ -n "$OUT" ] && : > "$OUT"

# ACE ships inside the OpenDDS prefix here; probed against the header
# the program includes rather than the prefix, because an OpenDDS built
# without TAO still has bin/opendds_idl and no ace/CDR_Stream.h.
ACE_INCLUDE="${ACE_INCLUDE:-$OPENDDS_HOME/include}"
ACE_LIB="${ACE_LIB:-$OPENDDS_HOME/lib}"
if [ ! -f "$ACE_INCLUDE/ace/CDR_Stream.h" ]; then
    printf 'ace: NOT PRESENT (ACE_INCLUDE=%s) — 7 rows not verified in this run\n' \
        "$ACE_INCLUDE" >&2
    [ $REQUIRE -eq 1 ] && exit 1
    exit 0
fi

WORK="${WORK:-$(mktemp -d "${TMPDIR:-/tmp}/unspec-fixed.XXXXXX")}"
mkdir -p "$WORK"

cat > "$WORK/ref.cpp" <<'CPP'
#include <ace/CDR_Stream.h>
#include <ace/CDR_Base.h>
#include <cstdio>

// The declared (digits, scale) travel with the row because they are the
// half ACE does not know: `ACE_CDR::Fixed::from_string` takes only the
// text, so a row that printed the bytes alone could not show where the
// value's digit count and the type's come apart.
static void one(unsigned digits, unsigned scale, const char *text) {
    ACE_CDR::Fixed value = ACE_CDR::Fixed::from_string(text);
    ACE_OutputCDR out;
    out << value;
    printf("fixed<%u,%u>\t%s\t", digits, scale, text);
    size_t n = 0;
    for (const ACE_Message_Block *b = out.begin(); b; b = b->cont())
        for (size_t i = 0; i < b->length(); i++, n++)
            printf("%02x", static_cast<unsigned char>(b->rd_ptr()[i]));
    printf("\t%zu\n", n);
}

int main() {
    one(5, 2, "123.45");    // the sign nibble, positive
    one(5, 2, "-123.45");   // and negative
    one(4, 2, "12.34");     // an even declared digit count takes a leading pad nibble
    one(1, 0, "7");         // one digit, one octet
    one(31, 0, "9999999999999999999999999999999");  // the IDL 4.2 maximum
    one(5, 5, "0.12345");   // scale equal to digits
    one(5, 2, "0.00");      // zero, where a value-driven digit count is shortest
    return 0;
}
CPP

if ! ( cd "$WORK" && g++ -std=c++17 -o ref ref.cpp \
        -I"$ACE_INCLUDE" -L"$ACE_LIB" -lACE -Wl,-rpath,"$ACE_LIB" > cc.log 2>&1 ); then
    echo "ace: the probe did not compile — see $WORK/cc.log" >&2
    grep -m6 'error:' "$WORK/cc.log" >&2
    exit 1
fi

"$WORK/ref" | while IFS= read -r line; do emit "$line"; done
exit 0
