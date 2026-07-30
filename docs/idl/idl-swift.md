<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → Swift codegen backend

`zerodds-idlc --swift` emits **native Swift** from OMG IDL: a self-contained
source file with a `Writer` (byte-identical to the `endpoints/swift` core) and,
per IDL `struct`, a Swift struct plus a `marshalXCDR(_ endian) -> [UInt8]`
method whose wire output matches the Rust goldens exactly.

Crate: [`crates/idl-swift`](../crates/idl-swift) · registered in `tools/idlc`
(`Backend::Swift`, flag `--swift`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --swift    # emits types.swift
```

## Mapping

| IDL | Swift |
|-----|-------|
| `struct` `@final` | struct + compact `marshalXCDR` (no DHEADER) |
| `struct` `@appendable` | struct + DHEADER-framed `marshalXCDR` |
| `octet` / `uint8` | `UInt8` |
| `short`..`long long` (signed) | `Int16`..`Int64` (`bitPattern` to the wire) |
| unsigned 16/32/64 | `UInt16` / `UInt32` / `UInt64` |
| `float` / `double` | `Float` / `Double` (`bitPattern`) |
| `boolean` / `char` | `Bool` / `UInt8` |
| `string` | `String` |
| `sequence<octet>` | `[UInt8]` |

Only `@mutable` unions and non-literal array/collection bounds currently raise
`IdlSwiftError::Unsupported`; unions, nested-struct members, maps, `long double`,
`wchar`, and `wstring` are emitted.

## Byte-identity

The `crates/idl-swift` suite runs string smoke tests in CI (job `idl-swift`, the
Rust image has no swiftc). The `byte_identity_vs_rust_goldens` test compiles and
runs the generated Swift (`swiftc`) when available — verified on macOS
(Swift 6.3): the generated `marshalXCDR` output equals `golden_le.bin` /
`golden_be.bin` byte-for-byte. The `endpoints-swift` job proves the same wire in
the `swift:6.0` image.
