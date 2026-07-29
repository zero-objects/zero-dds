<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-swift` v1.0 — Swift XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md) /
[`-julia`](zerodds-xcdr2-julia-1.0.md): the Swift binding of the XCDR2 wire — what
`zerodds-idlc --swift` emits and what the native `endpoints/swift` SDK provides.

## §1 Motivation

OMG has no IDL-to-Swift mapping. ZeroDDS defines a pure-Swift XCDR2 wire-core
(`endpoints/swift`) and a codegen backend (`crates/idl-swift`) that emits, per IDL
`struct`, a Swift `struct` plus a `marshalXCDR` method whose bytes equal the Rust
`zerodds-cdr` output. No C shim: `[UInt8]` carries the wire.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```swift
public struct Reading {
    public var id: UInt32
    public var value: Float
    public var label: String

    public func marshalXCDR(_ endian: Endianness) -> [UInt8] {
        var w = Writer(endian)
        w.putU32(id)
        w.putF32(value)
        w.putString(label)
        return w.bytes()
    }
}
```

## §3 Required API-Surface

`endpoints/swift/Sources/Zerodds/Zerodds.swift` (module `Zerodds`) MUST provide:
`Endianness` (`.little`, `.big`); `Writer`
(`putU8/putU16/putU32/putU64/putF32/putBytes/putString/putSeqU8`, `bytes`);
`Reader` (`getU8/getU16/getU32/getU64/getF32/getString/getSeqU8` — the byte-exact
inverse, `f32` via `Float(bitPattern:)`, `u64` via `UInt64`). The reader is a
mutable cursor. Generated per struct: `marshalXCDR(_ endian) -> [UInt8]`. Decode is
a `Reader` walk (§10). Generated decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-swift`)

Per IDL `struct`, `zerodds-idlc --swift` MUST emit a Swift `struct`, a `marshalXCDR`
method, and the self-contained `Writer`. Extensibility drives framing (§6);
unsupported constructs raise `IdlSwiftError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | Swift | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `Bool` | 1 byte |
| `octet`/`uint8` | `UInt8` | 1 byte |
| `char` | `UInt8` | 1 byte |
| `short`/`int16` | `Int16` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `UInt16` | 2 bytes LE, align 2 |
| `long`/`int32` | `Int32` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `UInt32` | 4 bytes LE, align 4 |
| `long long`/`int64` | `Int64` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `UInt64` | 8 bytes LE, align 4 |
| `float` | `Float` | 4 bytes IEEE-754 LE (`.bitPattern`) |
| `double` | `Double` | 8 bytes IEEE-754 LE |
| `string` | `String` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `[UInt8]` | uint32 count + raw bytes |

Byte order is an explicit parameter, so a big-endian target produces the same wire.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER (`uint32` body length + body).
`@mutable` — EMHEADER; not yet emitted → `Unsupported` (§11). The hand-written
`endpoints/swift` types are `@final`.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing (MD5 of key members' XCDR2-BE) is
runtime-provided; per-struct `keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/swift/Sources/Zerodds/Zerodds.swift` is the reference `Writer`/`Reader`.
`idl-swift` embeds a byte-identical `Writer` per generated file. Both byte-identical
to `zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-swift/tests/golden.rs::byte_identity_vs_rust_goldens`
  (verified on macOS `swiftc`; CI `idl-swift` runs the generation smoke tests — the
  Rust CI image has no swiftc).
- **Endpoint:** `endpoints/swift/Tests/ZeroddsTests` (`swift test` against the
  committed `testdata/golden_le.bin` / `golden_be.bin`) — CI job `endpoints-swift`
  (`swift:6.0` image).

## §10 Examples

- Sync: [`endpoints/swift/Sources/ZeroddsExampleSync/main.swift`](../../endpoints/swift/Sources/ZeroddsExampleSync/main.swift)
  — poll loop, full field decode.
- Async: [`endpoints/swift/Sources/ZeroddsExampleAsync/main.swift`](../../endpoints/swift/Sources/ZeroddsExampleAsync/main.swift)
  — `AsyncStream` (`for await body in reader.stream()`).
- Quickstart: [`endpoints/swift/QUICKSTART.md`](../../endpoints/swift/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope, uniform across all 17 idlc backends: generated
decode, per-struct `keyHash` from `@key`, `@mutable` EMHEADER, and
`wchar`/`wstring`/`map`/array/nested-struct/union/`long double` (raise
`Unsupported`). See the coverage doc's decision records.
