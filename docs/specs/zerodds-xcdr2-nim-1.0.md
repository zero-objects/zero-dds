<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-nim` v1.0 — Nim XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md) /
[`-zig`](zerodds-xcdr2-zig-1.0.md): the Nim binding of the XCDR2 wire — what
`zerodds-idlc --nim` emits and what the native `endpoints/nim` SDK provides.

## §1 Motivation

OMG has no IDL-to-Nim mapping. ZeroDDS defines a pure-Nim XCDR2 wire-core
(`endpoints/nim`) and a codegen backend (`crates/idl-nim`) that emits, per IDL
`struct`, a Nim `object` plus a `marshalXCDR` proc whose bytes equal the Rust
`zerodds-cdr` output.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```nim
type Reading* = object
  id*: uint32
  value*: float32
  label*: string

proc marshalXCDR*(self: Reading, endian: Endian): seq[byte] =
  var w = initWriter(endian)
  w.putU32(self.id)
  w.putF32(self.value)
  w.putString(self.label)
  w.bytes()
```

## §3 Required API-Surface

`endpoints/nim/zerodds.nim` MUST provide: `Endian` (`eLE`, `eBE`); `Writer`
(`putU8/putU16/putU32/putU64/putF32/putString/putSeqU8`, `bytes`); `Reader`
(`getU8/getU16/getU32/getU64/getF32/getString/getSeqU8` — the byte-exact inverse).
Generated per struct: `marshalXCDR(endian): seq[byte]`. Decode is a `Reader` walk
(§10). Generated decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-nim`)

Per IDL `struct`, `zerodds-idlc --nim` MUST emit a Nim `object`, a `marshalXCDR`
proc, and the self-contained `Writer`. Extensibility drives framing (§6);
unsupported constructs raise `IdlNimError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | Nim | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `bool` | 1 byte |
| `octet`/`uint8` | `uint8` | 1 byte |
| `char` | `char` | 1 byte |
| `short`/`int16` | `int16` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `uint16` | 2 bytes LE, align 2 |
| `long`/`int32` | `int32` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `uint32` | 4 bytes LE, align 4 |
| `long long`/`int64` | `int64` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `uint64` | 8 bytes LE, align 4 |
| `float` | `float32` | 4 bytes IEEE-754 LE (`cast[uint32]`) |
| `double` | `float64` | 8 bytes IEEE-754 LE |
| `string` | `string` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `seq[byte]` | uint32 count + raw bytes |

Byte order is an explicit parameter, so a big-endian target produces the same wire.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER (`uint32` body length + body).
`@mutable` — EMHEADER; not yet emitted → `Unsupported` (§11).

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing (MD5 of key members' XCDR2-BE) is
runtime-provided; per-struct `keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/nim/zerodds.nim` is the reference `Writer`/`Reader`. `idl-nim` embeds
a byte-identical `Writer` per generated file. Both byte-identical to `zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-nim/tests/golden.rs::byte_identity_vs_rust_goldens`
  (`nim c -r`) — CI job `idl-nim`.
- **Endpoint:** `endpoints/nim/test.nim` — CI job `endpoints-nim`.

## §10 Examples

- Sync: [`endpoints/nim/example_sync.nim`](../../endpoints/nim/example_sync.nim)
  — poll loop, full field decode.
- Async: [`endpoints/nim/example_async.nim`](../../endpoints/nim/example_async.nim)
  — `asyncdispatch` `await reader.recv()`.
- Quickstart: [`endpoints/nim/QUICKSTART.md`](../../endpoints/nim/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope, uniform across all 17 idlc backends: generated
decode, per-struct `keyHash` from `@key`, `@mutable` EMHEADER, and
`wchar`/`wstring`/`map`/array/nested-struct/union/`long double` (raise
`Unsupported`). See the coverage doc's decision records.
