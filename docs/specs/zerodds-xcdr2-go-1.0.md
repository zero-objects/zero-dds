<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-go` v1.0 — Go XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`zerodds-xcdr2-ts`](zerodds-xcdr2-ts-1.0.md): the Go binding of the
XCDR2 wire — what `zerodds-idlc --go` emits and what the native `endpoints/go`
SDK provides, so an IDL type round-trips byte-for-byte with every other binding.

## §1 Motivation

OMG has no IDL-to-Go language mapping and no Go DDS wire mapping. ZeroDDS defines
one: a pure-Go XCDR2 wire-core (`endpoints/go`) and a codegen backend
(`crates/idl-go`) that emits, per IDL `struct`, a Go struct plus a
`MarshalXCDR` method whose bytes equal the Rust `zerodds-cdr` output.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```go
type Reading struct {
    Id    uint32
    Value float32
    Label string
}

func (v Reading) MarshalXCDR(endian Endianness) []byte {
    w := NewWriter(endian)
    w.PutU32(v.Id)
    w.PutF32(v.Value)
    w.PutString(v.Label)
    return w.Bytes()
}
```

## §3 Required API-Surface

The wire-core (`endpoints/go`, package `zerodds`) MUST provide:

- `type Endianness` with `Little`, `Big`.
- `Writer` with `PutU8/PutBool/PutU16/PutU32/PutU64/PutF32/PutF64/PutBytes/
  PutString/PutSeqU8`, `Bytes()`.
- `Reader` with `GetU8/GetU16/GetU32/GetU64/GetF32/GetString/GetSeqU8` (the
  byte-exact inverse of the Writer).
- Generated per struct: `MarshalXCDR(endian) []byte` (encode). Decode is a
  hand-written `Reader` walk in the consumer (see §10); a generated
  `UnmarshalXCDR` is an open item (§11).
- Key hashing (`keyHash`) — see §7.

## §4 Codegen-Pflicht (`idl-go`)

Per IDL `struct`, `zerodds-idlc --go` MUST emit:

1. A Go `struct` with an exported field per IDL member (mapped per §5).
2. A `MarshalXCDR(endian Endianness) []byte` method.
3. The shared `Writer` wire-core (self-contained, so the file compiles with no
   external module).

Extensibility drives the framing (§6). Unions, nested-struct members, maps,
`long double`, `wchar`, `wstring`, and `@mutable` structs are emitted; only
`@mutable` unions and non-literal bounds raise `IdlGoError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | Go | Wire (XCDR2, align relative to buffer start, cap 4) |
|-----|-----|-----|
| `boolean` | `bool` | 1 byte |
| `octet` / `uint8` | `byte` / `uint8` | 1 byte |
| `char` | `byte` | 1 byte |
| `short`/`int16` | `int16` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `uint16` | 2 bytes LE, align 2 |
| `long`/`int32` | `int32` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `uint32` | 4 bytes LE, align 4 |
| `long long`/`int64` | `int64` | 8 bytes LE, align 4 (XCDR2) |
| `unsigned long long`/`uint64` | `uint64` | 8 bytes LE, align 4 (XCDR2) |
| `float` | `float32` | 4 bytes IEEE-754 LE (`math.Float32bits`) |
| `double` | `float64` | 8 bytes IEEE-754 LE (`math.Float64bits`) |
| `string` | `string` | uint32 (len+1) + UTF-8 bytes + NUL |
| `sequence<octet>` | `[]byte` | uint32 count + raw bytes |
| `sequence<primitive>` | `[]T` | uint32 count + T elements |

Byte order is an explicit parameter (the XCDR encapsulation flag), so a
big-endian target produces the same wire as an x86-64 host.

## §6 Extensibility

- `@final` — compact, no DHEADER.
- `@appendable` — a DHEADER: `uint32` body length + body bytes.
- `@mutable` structs — DHEADER + per-member EMHEADER (LC4) list, emitted.
  `@mutable` **unions** are not yet emitted → `Unsupported` (§11).

## §7 Key-Extraction

Keyed types: the key hash is the MD5 of the key members' XCDR2-BE serialisation
(DDSI-RTPS 2.5 §9.6.3.8). The Go SDK exposes this via a `KeyHash` helper over the
same `Writer` in BE mode; codegen of a per-struct `KeyHash` from `@key` members is
an open item (§11). Non-keyed types have a 16-zero-byte key.

## §8 Wire-Core

`endpoints/go/wire.go` (+ `reader_ext.go`) is the reference `Writer`/`Reader`.
`idl-go` embeds a byte-identical copy of the `Writer` in each generated file so
the output is self-contained. Both are byte-identical to `zerodds-cdr`.

## §9 Conformance

A binding is conformant iff, for the `@final` golden type (id `0xA1B2C3D4`,
kind `0x1234`, flags `0x5A`, value `3.5`, stamp `0x0102030405060708`, label
`"bay-12"`, raw `DE AD BE EF`), its encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-go/tests/golden.rs::byte_identity_vs_rust_goldens`
  (generates Go, `go run`, compares) — CI job `idl-go`.
- **Endpoint:** `endpoints/go/wire_test.go` + CI job `endpoints-go`.

## §10 Examples

- Sync: [`endpoints/go/example_sync`](../../endpoints/go/example_sync) — poll loop
  with a deadline, full field decode.
- Async: [`endpoints/go/example_async`](../../endpoints/go/example_async) —
  goroutine/channel `AsyncReader`.
- Quickstart: [`endpoints/go/QUICKSTART.md`](../../endpoints/go/QUICKSTART.md).

## §11 Errata + Open-Questions

- Generated `UnmarshalXCDR` (decode) — not emitted; consumers hand-write a
  `Reader` walk. Open.
- Per-struct generated `KeyHash` from `@key` — open (§7).
- `@mutable` unions and non-literal bounds — `Unsupported` in `idl-go` today.
  Unions, nested-struct members, `map<>`, arrays, `wchar`, `wstring`, `long
  double`, and `@mutable` structs are emitted.
