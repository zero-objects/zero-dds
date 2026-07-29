<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-zig` v1.0 — Zig XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`zerodds-xcdr2-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md):
the Zig binding of the XCDR2 wire — what `zerodds-idlc --zig` emits and what the
native `endpoints/zig` SDK provides, so an IDL type round-trips byte-for-byte.

## §1 Motivation

OMG has no IDL-to-Zig mapping. ZeroDDS defines a pure-Zig XCDR2 wire-core
(`endpoints/zig`) and a codegen backend (`crates/idl-zig`) that emits, per IDL
`struct`, a Zig struct plus a `marshalXCDR` method whose bytes equal the Rust
`zerodds-cdr` output.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```zig
pub const Reading = struct {
    id: u32,
    value: f32,
    label: []const u8,

    pub fn marshalXCDR(self: Reading, endian: Endian, alloc: std.mem.Allocator) ![]u8 {
        var w = Writer.init(alloc, endian);
        errdefer w.deinit();
        try w.putU32(self.id);
        try w.putF32(self.value);
        try w.putString(self.label);
        return try w.buf.toOwnedSlice();
    }
};
```

## §3 Required API-Surface

The wire-core (`endpoints/zig`, module `zerodds`) MUST provide:

- `pub const Endian = enum { little, big }`.
- `Writer` (allocator-backed `std.ArrayList`) with `putU8/putBool/putU16/putU32/
  putU64/putF32/putBytes/putString/putSeqU8`, `bytes()`.
- `Reader` with `getU8/getU16/getU32/getU64/getF32/getString/getSeqU8` (the
  byte-exact inverse of the Writer).
- Generated per struct: `marshalXCDR(endian, allocator) ![]u8`. Decode is a
  `Reader` walk in the consumer (§10). Generated decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-zig`)

Per IDL `struct`, `zerodds-idlc --zig` MUST emit: (1) a Zig `struct` with a field
per IDL member (§5), (2) `marshalXCDR(endian, allocator) ![]u8`, (3) the shared
`Writer` wire-core (self-contained). Extensibility drives the framing (§6);
unsupported constructs raise `IdlZigError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | Zig | Wire (XCDR2, align relative to buffer start, cap 4) |
|-----|-----|-----|
| `boolean` | `bool` | 1 byte |
| `octet` / `uint8` | `u8` | 1 byte |
| `char` | `u8` | 1 byte |
| `short`/`int16` | `i16` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `u16` | 2 bytes LE, align 2 |
| `long`/`int32` | `i32` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `u32` | 4 bytes LE, align 4 |
| `long long`/`int64` | `i64` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `u64` | 8 bytes LE, align 4 |
| `float` | `f32` | 4 bytes IEEE-754 LE (`@bitCast`) |
| `double` | `f64` | 8 bytes IEEE-754 LE (`@bitCast`) |
| `string` | `[]const u8` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `[]const u8` | uint32 count + raw bytes |

Signed integers reinterpret to the unsigned wire via `@bitCast`. Byte order is an
explicit parameter, so a big-endian target produces the same wire.

## §6 Extensibility

- `@final` — compact, no DHEADER.
- `@appendable` — a DHEADER: `uint32` body length + body bytes.
- `@mutable` — EMHEADER framing; not yet emitted → `Unsupported` (§11).

## §7 Key-Extraction

Non-keyed types have a 16-zero-byte key. Keyed key-hashing (MD5 of the key
members' XCDR2-BE serialisation) is provided by the DCPS runtime; per-struct
`keyHash` codegen from `@key` — §11.

## §8 Wire-Core

`endpoints/zig/src/zerodds.zig` is the reference `Writer`/`Reader`. `idl-zig`
embeds a byte-identical copy of the `Writer` in each generated file. Both are
byte-identical to `zerodds-cdr`.

## §9 Conformance

Conformant iff, for the `@final` golden type (id `0xA1B2C3D4`, kind `0x1234`,
flags `0x5A`, value `3.5`, stamp `0x0102030405060708`, label `"bay-12"`, raw
`DE AD BE EF`), the encoding equals `golden_le.bin` / `golden_be.bin`.

- **Codegen:** `crates/idl-zig/tests/golden.rs::byte_identity_vs_rust_goldens`
  (generates Zig, `zig run`, compares) — CI job `idl-zig`.
- **Endpoint:** `endpoints/zig/src/zerodds.zig` in-file tests + CI job `endpoints-zig`.

## §10 Examples

- Sync: [`endpoints/zig/example_sync.zig`](../../endpoints/zig/example_sync.zig)
  — poll (pull) loop, full field decode.
- Async: [`endpoints/zig/example_async.zig`](../../endpoints/zig/example_async.zig)
  — callback-reactor `AsyncReader`.
- Quickstart: [`endpoints/zig/QUICKSTART.md`](../../endpoints/zig/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope, uniform across all 17 idlc backends: generated
decode (`unmarshalXCDR`), per-struct `keyHash` from `@key`, `@mutable` EMHEADER,
and `wchar`/`wstring`/`map`/array/nested-struct/union/`long double` (raise
`Unsupported`). See the coverage doc's decision records.
