# The Java wire runtime

The counterpart of `crates/wire`, `runtime/c` and
`runtime/python/zerodds_wire` — a fourth implementation of DDS-XTypes 1.3
§7.4, not a binding to any of the other three. A Java column that reached
the Rust core through JNI would be byte-identical by construction and
would prove nothing about whether the two read the specification the same
way.

This package makes no decision about a type. Which members are written,
in what order, with which header and which length code is the layout
plan's answer and arrives here as arguments.

## What is here

| Class | What it carries |
| --- | --- |
| `Writer` | every write, the alignment rule, and the framing forms |
| `Reader` | its mirror, refusing every stream that cannot be true |
| `Float128` | IEEE 754 binary128, the type Java has no primitive for |
| `Md5` | the KeyHash digest of DDSI-RTPS 2.5 §9.6.3.8 |
| `Encapsulation` | the four bytes that announce an encoding |
| `Endianness`, `Encoding` | byte order and MAXALIGN |
| `PlCdr1Framing`, `PidFlagPolicy`, `ParameterRole` | how a PL_CDR1 parameter list is framed |
| `WstringLengthPolicy`, `WstringUnitOrder`, `WcharWidth`, `WcharFraming` | the `wchar`/`wstring` axes |
| `FixedDigitCount` | how many BCD digits a `fixed<P,S>` puts out |
| `WireException` | `Truncated`, `ImpossibleLength`, `NotUtf8`, `OutOfRange` |

Java 8 source level, no dependency beyond the platform.

## The four things Java makes easy to get wrong

**There are no unsigned integers.** Every method says in its own
documentation whether its argument is a *number* that is range-checked —
`writeU8(int)`, `writeU16(int)`, `writeU32(long)` — or a *bit pattern*
whose every value is admissible, which is `writeU64(long)` and
`writeBitmask` at a bound of 64. `writeU32` takes a `long` for exactly
one reason: an `int` parameter would make `3735928559` arrive as
`-559038737` with the loss already done. A silent overflow here puts a
different number on the wire and reports success.

**Alignment is `MIN(N, MAXALIGN)` counted from the origin.** Eight under
XCDR1 and four under XCDR2, and the origin is where a PL_CDR1
parameter's block began — not zero, and never the value's own start.
`endpoints/java/Zdw.java` caps it at four unconditionally, which is
correct for the XCDR2-only core it is and wrong for anything else.
`WireSelfTest` checks an eight-byte field after a one-byte field at both
caps, in both directions, and asserts that a reader at the wrong cap
returns a *different* number.

**`String.length()` is neither bytes nor characters.** It is UTF-16 code
units, so it is the length of neither a `string` nor a `wstring` on the
wire. A `string`'s length counts octets of UTF-8 and includes the NUL; a
`wstring`'s counts, under the specification policy, the octets its code
units occupy, so `"𝄞"` is a length of four.

**`String.getBytes(UTF_8)` does not fail.** It replaces an unpaired
surrogate with `?` and reports success. `Writer` encodes through a
reporting `CharsetEncoder` and raises `WireException.NotUtf8` instead,
and refuses a lone surrogate in a `wstring` for the same reason.

## binary128

Java has no primitive wider than `double`. `Float128` answers that with
the bit pattern for the wire and a `BigDecimal` for the value, because
the conversion between them is exact — every finite binary128 is
`significand * 2^exponent` with the significand an integer under 2^113,
and that is a terminating decimal `BigDecimal` holds in full.

One pattern does not survive a round trip through `BigDecimal`:

    0x80000000000000000000000000000000    negative zero

`BigDecimal` has no signed zero. The value is not lost — negative zero
and positive zero are numerically equal and IEEE 754 says so — but the
sixteen bytes are, which on a wire is the thing that matters. So
`Float128` stores the *pattern*, `equals` compares it, and the text form
carries the sign: `fromDecimalString("-0.0")` returns it. Infinities and
NaNs are the same shape of case and `toBigDecimal` refuses them by name
rather than inventing a decimal.

There is no arithmetic. A caller that wants to compute takes the
`BigDecimal`, computes in it, and comes back.

## Running the tests

```sh
runtime/java/run-tests.sh
```

Compiles with `--release 8 -Xlint:all -Werror` and runs three checks.

`Float128SelfTest` reads `crates/float128/tests/oracle/patterns.txt` —
518 bit patterns the Rust, C and Python columns already agree on, and
which GCC's own `__float128` was measured against — and asserts that this
class prints the same text and reads it back to the same pattern.

`WireSelfTest` covers alignment at both MAXALIGNs, every primitive in
both byte orders including `u64` at `0xFFFFFFFFFFFFFFFF`, strings and
`wstring`s and `fixed`, DHEADER, every EMHEADER length code including the
short-code rewrite, the short and extended parameter headers including
the promotion at a length over `0xFFFF`, and RFC 1321's own MD5 vectors.

`CorpusCrossCheck` is the one that is not this column's own opinion:
three pure-primitive cases from `tests/interop-corpus/` are written here
by hand from their `type.idl` and `values.json`, and the bytes are
compared with what the Rust column recorded in
`tests/interop-results/<case>/zerodds-rust/` under `plain-cdr2.le` and
`plain-cdr1.be` — the two encodings that disagree about whether an
eight-byte field aligns to four or to eight.

See `docs/float128-column.md` for the vendor measurements the binary128
class answers.
