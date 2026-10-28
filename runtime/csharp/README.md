# The C# wire runtime

The counterpart of `crates/wire`, `runtime/c`, `runtime/cpp`,
`runtime/java` and `runtime/python/zerodds_wire` — a sixth
implementation of DDS-XTypes 1.3 §7.4, not a binding to any of the other
five. A C# column that reached the Rust core through P/Invoke would be
byte-identical by construction and would prove nothing about whether the
two read the specification the same way.

This assembly makes no decision about a type. Which members are written,
in what order, with which header and which length code is the layout
plan's answer and arrives here as arguments.

## What is here

| File | What it carries |
| --- | --- |
| `Writer.cs` | every write, the alignment rule, and the framing forms |
| `Reader.cs` | its mirror, refusing every stream that cannot be true |
| `Float128.cs` | IEEE 754 binary128, the type .NET has no primitive for |
| `Md5.cs` | RFC 1321, for the KeyHash of DDSI-RTPS 2.5 §9.6.3.8 |
| `Encapsulation.cs` | the four bytes that announce an encoding |
| `Endianness.cs`, `Encoding.cs` | byte order and MAXALIGN |
| `Handles.cs` | `DHeader`, `EmHeader`, `Pid`, their read forms, and `Status` |
| `PlCdr1Framing.cs` | how a PL_CDR1 parameter list is framed |
| `Policies.cs` | the `wchar`/`wstring` axes, `fixed`'s digit count, `@try_construct` |
| `IdlName.cs` | the attribute a generated member's IDL spelling lives in |
| `WireException.cs` | `Truncated`, `ImpossibleLength`, `NotUtf8`, `OutOfRange` |

`net8.0`, `TreatWarningsAsErrors`, and no package reference at all: a
measurement host has the SDK and no feed.

## The four decisions this column makes, and why

### 1. The unsigned types are real, so the range checks are the compiler's

This is the whole difference from the Java column. There,
`writeU32(long)` takes the next signed type up and range-checks at run
time, because an `int` parameter would make `3735928559` arrive as
`-559038737` with the loss already done — and `writeU64` cannot be a
number at all and is documented as a bit pattern.

Here `WriteU32(uint)` takes a `uint`, `WriteU64(ulong)` takes a `ulong`,
and `18446744073709551615` is a number. The value that does not fit is
refused by `csc` rather than by a throw. What is left of the range
checking is the ranges no C# type expresses: a `char` below U+0100, an
`@bit_bound` between 1 and 32, a BCD field that fits `fixed<P,S>`.

### 2. `long double` is a sixteen-octet carrier and **not** `decimal`

`Float128` holds the pattern as two `ulong` halves and converts to and
from an exact decimal. `decimal` was considered and is not a binary128:

| | binary128 | `decimal` |
| --- | --- | --- |
| base | two | ten |
| significand | 113 bits, ~34 decimal digits | 96 bits, 28–29 digits |
| exponent range | ±4932 | 0 to −28 |
| signed zero | yes | no |
| infinity, NaN | yes | no |

Three corpus values settle it on their own: `1e4000` is a finite
binary128 and is outside `decimal` by a factor of 10^3971; `-0.0` and
`+0.0` are different sixteen bytes on the wire and one `decimal`;
`primitive-longdouble` carries five values no `double` holds. A member
typed against `decimal` would put a different number on the wire and
report success.

The namespace is `ZeroDds.Idl` and not `ZeroDds.Wire`, deliberately:
`crates/lang-rules/src/adapter/csharp.rs` writes
`global::ZeroDds.Idl.Float128` as the member type for a `long double`,
and a codec that met a second type of the same shape under a different
name would not compile against the declarations. What is here is a
**superset** of the struct that materialiser emits into a standalone
file — the same two fields and the same constructor, plus the
conversions — and `tools/wireprobe/wireprobe-zerodds-csharp-gen` drops
the generated copy when it compiles against this assembly, because two
declarations of one name is CS0101.

### 3. `fixed<P,S>` is a `string` and **not** `decimal` either

Same table, one row of it: IDL 4.2 §7.4.1.4.4 allows 31 digits and
`decimal` carries 28 to 29. The corpus case `fixed-scales` declares
`fixed<31,10>` and its value `-12345678901234567890.1234567890` has
thirty significant digits, which a `decimal` rounds — measured, not
inferred.

Beyond the digits there is the axis. `FixedDigitCount.ValuePrecision`
exists to measure how many digits a peer writes into the BCD run —
omniORB and this project write the declared `P`, JacORB as many as the
value has, ACE/TAO (`FixedDigitCount.ValueStringDigits`) the digits of
the string it parsed — and a holder that normalises its
leading zeros away has already decided that answer. `WriteFixed` takes
the decimal text and does not parse it through `decimal.Parse`, which
would accept an exponent and normalise a scale.

### 4. MD5 is RFC 1321 by hand and not `System.Security.Cryptography`

Not distrust of the platform. `MD5.Create()` is unavailable under a
FIPS-enforcing policy: the managed implementation throws
`InvalidOperationException` and the OS refuses the native one. A KeyHash
that stops working because a machine was put in FIPS mode is a wire
feature depending on a security policy it has nothing to do with —
DDSI-RTPS 2.5 §9.6.3.8 names MD5 to identify an instance, not to sign
one. `SelfTest` checks the arithmetic against RFC 1321 §A.5's own
vectors, so the assumption is asserted rather than commented.

## The three things .NET makes easy to get wrong

**`string.Length` is neither bytes nor characters.** It is UTF-16 code
units, so it is the length of neither a `string` nor a `wstring` on the
wire. A `string`'s length counts octets of UTF-8 and includes the NUL; a
`wstring`'s counts, under the specification policy, the octets its code
units occupy, so `"𝄞"` is a length of four.

**`Encoding.UTF8` does not fail.** It replaces an unpaired surrogate
with U+FFFD and reports success — a different character on the wire,
silently. `Writer` encodes through a `UTF8Encoding` built with
`throwOnInvalidBytes: true` and raises `WireException.NotUtf8` instead.

**Alignment is `MIN(N, MAXALIGN)` counted from the origin.** Eight under
XCDR1 and four under XCDR2, and the origin is where a PL_CDR1
parameter's block began — not zero, and never the value's own start.
`SelfTest` checks an eight-byte field after a one-byte field at both
caps and asserts that a reader at the wrong cap returns a *different*
number rather than an error.

## Running the tests

```sh
runtime/csharp/run-tests.sh
```

Silent when everything holds. Three sections, and only the last two are
worth anything on their own:

- the wire checks assert what this runtime believes — alignment at both
  MAXALIGNs, every primitive in both byte orders including `u64` at
  `0xFFFFFFFFFFFFFFFF`, strings and `wstring`s and `fixed`, DHEADER,
  the EMHEADER length codes including the short-code rewrite, the short
  and extended parameter headers including the promotion at a length
  over `0xFFFF`, and RFC 1321's own MD5 vectors;
- **the binary128 oracle** reads
  `crates/float128/tests/oracle/patterns.txt` — 518 bit patterns the
  Rust, C, Java and Python columns already agree on, and which GCC's own
  `__float128` was measured against — and asserts that each shortest
  decimal reads back here to the same sixteen bytes;
- **the corpus cross-check** writes three pure-primitive cases by hand
  from their `type.idl` and `values.json` and compares the octets with
  what the Rust column recorded in
  `tests/interop-results/<case>/zerodds-rust/`, under `plain-cdr2.le`
  and `plain-cdr1.be` — the two encodings that disagree about whether an
  eight-byte field aligns to four or to eight.

The column itself is measured by
`tools/wireprobe/wireprobe-csharp-corpus` over the whole interop corpus,
and `crates/wire-probe/tests/csharp_column_gate.rs` runs the handful
that separate the things a column can get wrong on every `cargo test`.

## Package delivery

`ZeroDds.Wire` is the canonical generated-code runtime package, version
`1.0.0-rc.7`. The native `ZeroDDS` package references it transitively alongside
`ZeroDDS.Cdr`. Wire-only consumers can reference `ZeroDds.Wire` directly.

```sh
dotnet pack runtime/csharp/ZeroDds.Wire.csproj -c Release -o target/nuget
```

This prepares a local package, not a registry publication. The separate
native DDS library is not included in this runtime package.
