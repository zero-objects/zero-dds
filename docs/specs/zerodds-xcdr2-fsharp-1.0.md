<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-fsharp` v1.0 — F# XCDR2 TypeSupport

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md):
the F#/.NET binding of the XCDR2 wire — the native `endpoints/fsharp` SDK, and
how IDL types reach F#.

## §1 Motivation

OMG has no IDL-to-F# mapping. ZeroDDS provides a pure-F#/.NET XCDR2 wire-core
(`endpoints/fsharp`), byte-identical to the Rust core.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```fsharp
type Reading = { Id: uint32; Value: float32; Label: string }

let marshal (r: Reading) (endian: Endian) =
    let w = Writer(endian)
    w.PutU32(r.Id)
    w.PutF32(r.Value)
    w.PutString(r.Label)
    w.Bytes()
```

## §3 Required API-Surface

`endpoints/fsharp/zerodds.fs` (module `ZeroDDS`) MUST provide: `Endian` (`LE`,
`BE`); `Writer` (`PutU8/PutU16/PutU32/PutU64/PutF32/PutBytes/PutString/PutSeqU8`,
`Bytes`); `Reader` (`GetU8/GetU16/GetU32/GetU64/GetF32/GetString/GetSeqU8` — the
byte-exact inverse, `f32` via `BitConverter`). Decode is a `Reader` walk (§10);
generated decode / key hash — §11.

## §4 Codegen (inherits `idl-csharp`)

F# has **no dedicated idl backend**: it is .NET-native and consumes the
`zerodds-idlc --csharp` output directly (.NET interop) — the generated C#
TypeSupport is callable from F# unchanged, so a separate `idl-fsharp` would only
duplicate it (see `docs/async/idlc-coverage.md`). The native `endpoints/fsharp`
wire-core is byte-identical, so hand-written F# types (§2) and idl-csharp-generated
types share the same wire.

## §5 Wire-Type-Mapping

| IDL | F# | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `bool` | 1 byte |
| `octet`/`uint8` | `byte` | 1 byte |
| `char` | `char` | 1 byte |
| `short`/`int16` | `int16` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `uint16` | 2 bytes LE, align 2 |
| `long`/`int32` | `int32` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `uint32` | 4 bytes LE, align 4 |
| `long long`/`int64` | `int64` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `uint64` | 8 bytes LE, align 4 |
| `float` | `float32` | 4 bytes IEEE-754 LE (`BitConverter`) |
| `double` | `float` | 8 bytes IEEE-754 LE |
| `string` | `string` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `byte[]` | uint32 count + raw bytes |

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER. `@mutable` — EMHEADER; via idl-csharp.
The hand-written `endpoints/fsharp` types are `@final`.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing is runtime/idl-csharp-provided.

## §8 Wire-Core

`endpoints/fsharp/zerodds.fs` is the reference `Writer`/`Reader`, byte-identical
to `zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Endpoint:** `endpoints/fsharp/test.fsx` (`dotnet fsi`) — CI job `endpoints-fsharp`.
- **Codegen:** inherited from `crates/idl-csharp/tests` (spec `zerodds-xcdr2-csharp`).

## §10 Examples

- Sync: [`endpoints/fsharp/example_sync.fsx`](../../endpoints/fsharp/example_sync.fsx)
  — poll loop, full field decode.
- Async: [`endpoints/fsharp/example_async.fsx`](../../endpoints/fsharp/example_async.fsx)
  — `MailboxProcessor` agent (`let! body = reader.RecvAsync()`).
- Quickstart: [`endpoints/fsharp/QUICKSTART.md`](../../endpoints/fsharp/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope for the hand-written wire-core, uniform across all
endpoints: generated decode, per-struct `keyHash`, and `@mutable`/`wchar`/
`wstring`/`map`/array/nested/union — provided via the idl-csharp codegen path
where needed. See the coverage doc's decision records.
