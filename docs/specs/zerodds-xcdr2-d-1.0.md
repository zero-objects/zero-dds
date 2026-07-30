<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-d` v1.0 — D XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md) /
[`-zig`](zerodds-xcdr2-zig-1.0.md) / [`-nim`](zerodds-xcdr2-nim-1.0.md): the D
binding of the XCDR2 wire — what `zerodds-idlc --d` emits and what the native
`endpoints/d` SDK provides.

## §1 Motivation

OMG has no IDL-to-D mapping. ZeroDDS defines a pure-D XCDR2 wire-core
(`endpoints/d`) and a codegen backend (`crates/idl-d`) that emits, per IDL
`struct`, a D struct plus a `marshalXCDR` method whose bytes equal the Rust
`zerodds-cdr` output.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```d
struct Reading {
    uint id;
    float value;
    string label;

    ubyte[] marshalXCDR(Endian endian) {
        auto w = Writer(endian);
        w.putU32(id);
        w.putF32(value);
        w.putString(label);
        return w.bytes();
    }
}
```

## §3 Required API-Surface

`endpoints/d/zerodds.d` MUST provide: `enum Endian { LE, BE }`; `Writer`
(`putU8/putU16/putU32/putU64/putF32/putF64/putBytes/putString/putSeqU8`,
`bytes`); `Reader` (`getU8/getU16/getU32/getU64/getF32/getString/getSeqU8` — the
byte-exact inverse). Generated per struct: `marshalXCDR(Endian) ubyte[]`. Decode
is a `Reader` walk (§10). Generated decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-d`)

Per IDL `struct`, `zerodds-idlc --d` MUST emit a D `struct`, a `marshalXCDR`
method, and the self-contained `Writer`. Extensibility drives framing (§6);
unsupported constructs raise `IdlDError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | D | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `bool` | 1 byte |
| `octet`/`uint8` | `ubyte` | 1 byte |
| `char` | `char` | 1 byte |
| `short`/`int16` | `short` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `ushort` | 2 bytes LE, align 2 |
| `long`/`int32` | `int` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `uint` | 4 bytes LE, align 4 |
| `long long`/`int64` | `long` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `ulong` | 8 bytes LE, align 4 |
| `float` | `float` | 4 bytes IEEE-754 LE (`*cast(uint*)&v`) |
| `double` | `double` | 8 bytes IEEE-754 LE |
| `string` | `string` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `ubyte[]` | uint32 count + raw bytes |

Byte order is an explicit parameter, so a big-endian target produces the same wire.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER (`uint32` body length + body).
`@mutable` structs — DHEADER + per-member EMHEADER (LC4) list. `@mutable` unions are not yet emitted → `Unsupported` (§11).

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing (MD5 of key members' XCDR2-BE) is
runtime-provided; per-struct `keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/d/zerodds.d` is the reference `Writer`/`Reader`. `idl-d` embeds a
byte-identical `Writer` per generated file. Both byte-identical to `zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-d/tests/golden.rs::byte_identity_vs_rust_goldens`
  (`gdc`) — CI job `idl-d`.
- **Endpoint:** `endpoints/d/test.d` — CI job `endpoints-d`.

## §10 Examples

- Sync: [`endpoints/d/example_sync.d`](../../endpoints/d/example_sync.d) — poll
  loop, full field decode.
- Async: [`endpoints/d/example_async.d`](../../endpoints/d/example_async.d) —
  `std.concurrency` actor (`feed` / `recv`).
- Quickstart: [`endpoints/d/QUICKSTART.md`](../../endpoints/d/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope: generated decode and per-struct `keyHash` from
`@key`. `@mutable` unions and non-literal bounds also raise `Unsupported`;
`@mutable` structs, unions, `wchar`, `wstring`, `map`, arrays, nested-struct
members, and `long double` are emitted. See the coverage doc's decision records.
