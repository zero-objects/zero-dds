<!-- SPDX-License-Identifier: Apache-2.0 -->
# `runtime/cpp` — the C++ wire runtime

The C++ column of the XCDR wire rules: a fourth reading of DDS-XTypes 1.3
§7.4 beside `crates/wire`, `runtime/c` and `runtime/python/zerodds_wire`.

It is **not** a binding. Nothing here calls into the C runtime, no
translation unit includes `zerodds_wire.h`, and no function delegates to
another column. Every clause is implemented from the specification text, so
that a difference between this column and another is a difference in how two
people read the clause — which is the only reason to have four.

C++17, standard library only, no external dependency.

## Layout

```
include/zerodds/wire.hpp   the whole public surface
src/writer.cpp             the writer, the errors, the dialect tables, UTF-8 decode
src/reader.cpp             the reader and UTF-8 encode
src/float128.cpp           IEEE 754 binary128: bytes, double, exact decimal
src/md5.cpp                RFC 1321
tests/selftest.cpp         the self-test
tests/corpus_bytes.cpp     three interop-corpus cases against the recorded bytes
```

## Building

```
make                       build the objects, the archive and both test binaries
make test                  build and run the self-test
make corpus                build and run the byte comparison
make check                 both
make CXX=g++-16 check      any compiler
```

Or as a CMake subproject (`CMakeLists.txt`): `zerodds::zerodds-wire-cpp`, a
static library of the same four translation units, the way `crates/cpp`
(`zerodds-cpp`, the DDS-PSM-Cxx binding) and `zerodds-idlc generate --lang
c++ --cpp-mapping native`'s own output link it. The root `CMakeLists.txt` configures this
directory before `crates/cpp`. `ctest` from a zerodds CMake build runs the
same `selftest`/`corpus_bytes` binaries the Makefile does.

The warning set is the gate:

```
-std=c++17 -O1 -Wall -Wextra -Wpedantic -Wconversion -Wsign-conversion -Werror
```

`-Wconversion` and `-Wsign-conversion` are in it because every silent
narrowing this runtime could commit is a byte on the wire nobody asked for.

The self-test is silent on success and exits 0. A failure names the line, the
offset the bytes first differ at, and both hex strings.

## Using it

```cpp
#include "zerodds/wire.hpp"
using namespace zerodds::wire;

Writer w(Endianness::Little, max_alignment(Encoding::PlainCdr2));
w.write_bool(true);
w.write_u64(0x0102030405060708ull);
std::vector<std::uint8_t> payload = w.finish();

Reader r(payload, Endianness::Little, max_alignment(Encoding::PlainCdr2));
bool flag = r.read_bool();
std::uint64_t value = r.read_u64();
```

The encapsulation header is separate, because it frames a sample rather than
being part of one — a KeyHash holder is written by the same writer and
carries no header at all:

```cpp
std::array<std::uint8_t, 4> header = encapsulation_header(
    Encoding::PlainCdr2, Endianness::Little, encapsulation_padding(payload.size()));
```

## The traps, and where they are answered

**The alignment cap.** `align_to` takes `MIN(align, max_align)` and counts
the offset from `origin_`, not from zero and not from the start of the value.
`max_align` is 8 under XCDR1 and 4 under XCDR2 — §7.4.1's `MAXALIGN`. A
runtime that padded an eight-byte primitive to eight under XCDR2 puts four
bytes on the wire that no peer expects, and one that counted from the value's
own start puts a `long` after a `char` at offset 1. `Writer::align_to`,
`Reader::align_to`; `selftest.cpp:alignment()` checks an eight-byte field
after a one-byte field under both caps, in both directions, and checks that
the same bytes read under the wrong cap run off the end.

**A PL_CDR1 parameter value moves the origin.** "Each element, or parameter,
within a parameter list data structure is simply a CDR-encapsulated block of
data" — §7.4.1.2. `Writer::begin_parameter_value` replaces the origin and
`finish_pid` restores it *before* aligning to 4, because the trailing padding
is measured from the enclosing stream and the value's internal padding is
not. `Reader::begin_parameter_value` / `finish_parameter_value` are the
mirror.

**EMHEADER length codes 5 to 7.** Their NEXTINT *is* the member's own leading
word — §7.4.3.4.2: it "shall be reused also as part of the serialized
member". `write_emheader` reserves no placeholder for them, and
`read_emheader` reports the word and then puts the position back before it,
so the member's decoder still sees it.

**A string's length counts the NUL and counts octets.** `write_string` writes
`value.size() + 1`, and the size is the UTF-8 octet count and never a
character count.

**A `wstring` under `Specification` carries no BOM and no terminator**, and
its length is the **octet** count — §7.4.1.1.2 forbids both outright and
states the count as "twice the number of characters", the clause's own way of
saying that a code point outside the Basic Multilingual Plane takes two code
units.

**An out-of-range integer is refused.** The typed writers take the widest
natural type (`std::int64_t` / `std::uint64_t`) and range-check, so a caller
that hands `70000` to `write_u16` gets an `OutOfRange` naming
`an unsigned short in [0, 65535], not 70000` rather than `4464` on the wire.
That is where this column follows Python rather than C: an `std::uint16_t`
parameter would move the truncation to the call site, where this library
cannot see it and the caller's warning flags may not either.

## Decisions this column made

- **`wstring` input is UTF-8.** One representation, chosen because it is what
  `zdw_write_wstring_utf8` takes and what generated code has to hand:
  `std::wstring` is 16 bits on Windows and 32 elsewhere, and `std::u16string`
  would push the surrogate rule §7.4.1.1.2's byte count depends on out of the
  runtime and into its callers. `write_wstring` takes `std::string_view` and
  `read_wstring` returns `std::string`; a surrogate encoded in UTF-8 (CESU-8)
  is refused at both ends.
- **Errors are exceptions**, not a sticky status. `WireError` carries `at()`,
  and the subclasses carry exactly what the Python column's carry:
  `Truncated::needed()`/`available()`, `ImpossibleLength::claimed()`/
  `available()`, `OutOfRange::expected()`, `NotUtf8`. The message text is the
  Python message text, so a failure in one column can be grepped for in
  another. The writer owns a `std::vector`, so no failure is about running
  out of room and nothing is ever half-written.
- **A length code above 7 is refused**, where the Python column masks it with
  `& 0x7`. A masked code writes a frame that reads back as a different member
  extent and says nothing, which is the failure class this project treats as
  the worst one. This is the one place the C++ column is deliberately
  stricter than its reference; `write_emheader` throws `OutOfRange`.
- **`Float128` is never a `long double`.** On the three platforms this
  project builds for that type is three different formats — `LDBL_MANT_DIG`
  64 on Linux x86-64, 113 on Linux aarch64, 53 on macOS arm64 — and two DDS
  vendors memcpy the object onto the wire, so the same IDL and the same value
  give three different payloads. Nothing here reads the bytes of one. The
  decimal conversions are exact in both directions and go through an integer
  bignum, never through a `double`: a two-step parse rounds twice, and double
  rounding gets the last bit wrong for exactly the values this type exists to
  carry.
- **MD5 is implemented here**, not linked in. DDSI-RTPS 2.5 §9.6.3.8 names it
  for the KeyHash of a holder longer than sixteen bytes; it is not a security
  claim, and a wire runtime that dragged in a TLS dependency to name an
  instance would be unlinkable on the targets this project cares about.

## What the tests cover

`tests/selftest.cpp`:

- alignment at both caps, an eight-byte field after a one-byte field under
  each, and the same bytes misread under the wrong cap;
- every primitive round-tripped in both byte orders, plus the whole
  `primitive-mixed` field sequence as a byte string in XCDR2/LE and XCDR1/BE;
- out-of-range refusals, a boolean of 2, a truncated read with its
  `needed`/`available`;
- `string`, bounded `string` cut at a character boundary, an impossible
  length, non-UTF-8 bytes;
- `wstring` with a non-BMP code point across five length policies and all
  four unit orders, the byte-order mark in both directions, and the
  character-count-over-two-octet-units refusal;
- `fixed` negative, leading-zero, negative zero, odd digit count, value
  precision, `fixed<31,2>`, and the four refusals;
- DHEADER; EMHEADER LC0 to LC7 including the short-code rewrite and its
  absence; PID short, extended, the flag policies of all three vendors, the
  reserved range, `PID_IGNORE`, and the promotion at a value length above
  `0xFFFF`;
- `Float128` round-trip of `1 + 2^-112`, both byte orders as the whole
  sixteen bytes reversed, exact-decimal print and parse as inverses over the
  subnormal and the largest-finite ends, and the five corpus decimals;
- MD5 against the whole RFC 1321 appendix A.5 suite plus the block-boundary
  lengths, and both KeyHash forms;
- the ten encapsulation identifiers and the padding count;
- all eight `wstring` cells reachable by name and by coordinate.

`tests/corpus_bytes.cpp` writes three pure-primitive corpus cases with this
`Writer` and compares against the recorded foreign payloads under
`tests/interop-results/<case>/zerodds-rust/`. It reports the first differing
offset when they differ.
