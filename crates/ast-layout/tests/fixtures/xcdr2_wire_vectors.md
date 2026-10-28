# OMG XCDR2 wire vectors V-1 … V-12

Transcribed from the oracle at commit `e1f3af895`,
`crates/cdr/tests/xcdr2_wire_vectors.rs`, which pins these bytes against
OMG XTypes 1.3 §7.4 and documents them in
`docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6.

This file records what the specification says the bytes are, independent
of any implementation. `tests/wire_vectors.rs` asserts that the layout
planner predicts them. The oracle predates this rebuild and nothing here
produced it — a gate that compares an implementation against its own
output proves nothing.

All bytes are little-endian PLAIN_CDR2 payload, with no RTPS
representation header.

---

## V-1 — empty `@final`

```idl
@final struct Empty {};
```

Payload: zero bytes. No DHEADER.

## V-2 — `@final` two int32

```idl
@final struct Point { long x; long y; };
```

Sample `{ x = 1, y = -2 }`, 8 bytes:

```
01 00 00 00  FE FF FF FF
```

Member offsets 0 and 4. Extent 8.

## V-3 — `@final` mixed primitives

```idl
@final struct All {
  boolean b; octet o; short s; unsigned short us;
  long l; unsigned long ul; long long ll; unsigned long long ull;
  float f; double d;
};
```

48 bytes. The oracle's own offset table:

| member | offset | size | note |
|---|---|---|---|
| `b` | 0 | 1 | |
| `o` | 1 | 1 | |
| `s` | 2 | 2 | already aligned |
| `us` | 4 | 2 | |
| `l` | 8 | 4 | pad 2 |
| `ul` | 12 | 4 | |
| `ll` | 16 | 8 | already aligned |
| `ull` | 24 | 8 | |
| `f` | 32 | 4 | |
| `d` | 40 | 8 | **pad 4 to align(8)** |

**Total 48.** The last row is the load-bearing one: under PLAIN_CDR2 a
64-bit primitive aligns to 8, not to 4. An implementation that caps
alignment at four computes 44 and is wrong on the wire.

## V-4 — `@final` string

```idl
@final struct Message { string text; };
```

Sample `text = "hello"`, 10 bytes: `u32` length 6 including the NUL,
then the content and the NUL.

```
06 00 00 00  68 65 6C 6C 6F 00
```

Extent unbounded: the written length varies with the value.

## V-5 — `@final` sequence of int32

```idl
@final struct Bag { sequence<long> ids; };
```

Sample `ids = [1, 2, 3]`, 16 bytes: `u32` count then the elements.

```
03 00 00 00  01 00 00 00  02 00 00 00  03 00 00 00
```

## V-6 — `@final` sequence of string

```idl
@final struct Tags { sequence<string> tags; };
```

19 bytes. The element is not a primitive, which is what makes this
member LC5 under the compact framing policy.

## V-7 — `@final` nested modules

```idl
module Outer { module Inner { @final struct S { long v; }; }; };
```

Sample `v = 1234`, 4 bytes:

```
D2 04 00 00
```

Nesting changes the type name, not the wire form.

## V-8 — `@final` keyed

```idl
@final struct Sensor { @key long id; double value; };
```

Payload 16 bytes. The key holder is the `@key` member alone, four bytes,
big-endian:

```
00 00 00 2A
```

KeyHash is that holder zero-padded to sixteen bytes — DDSI-RTPS 2.5
§9.6.3.8, not MD5, because the holder fits.

## V-9 — `@appendable`

```idl
@appendable struct A { long a; long b; };
```

12 bytes: a four-byte DHEADER of 8, then the body.

```
08 00 00 00  01 00 00 00  02 00 00 00
```

## V-10 — `@mutable` with two members

```idl
@mutable struct M { @id(1) long a; @id(2) string b; };
```

31 bytes:

```
1B 00 00 00              DHEADER, body = 27
01 00 00 40              EMHEADER: lc=4, id=1
04 00 00 00              NEXTINT = 4
2A 00 00 00              int32 = 42
02 00 00 40              EMHEADER: lc=4, id=2
07 00 00 00              NEXTINT = 7
03 00 00 00 68 69 00     string "hi"
```

**Both members carry lc=4**, including the four-byte int32. The oracle
notes this as "the more general form spec-allowed by XTypes
§7.4.3.4.2": an explicit NEXTINT throughout, rather than the LC0…LC3
size codes or the LC5 optimisation.

The planner reproduces this under `LengthCodePolicy::UniversalNextInt`.
Under `LengthCodePolicy::Compact` the same type frames as LC2 and LC5 —
also conformant, and not the same bytes. That is why the policy is an
explicit part of the layout key rather than a hidden default: two
implementations that disagree about it do not interoperate.

## V-11a — `@mutable` optional, present

```idl
@mutable struct O { @id(1) @optional long maybe; };
```

16 bytes:

```
0C 00 00 00   DHEADER = 12
01 00 00 40   EMHEADER: lc=4, id=1
04 00 00 00   NEXTINT = 4
07 00 00 00   int32 = 7
```

## V-11b — `@mutable` optional, absent

4 bytes: a DHEADER of zero, member omitted entirely.

```
00 00 00 00
```

The member carries a header when present and none when absent, which is
how the reader is told which.

## V-12 — empty `@mutable`

```idl
@mutable struct EmptyMutable {};
```

A DHEADER of zero and nothing else. The DHEADER is present even at zero,
unlike `@final`, which has none at all.
