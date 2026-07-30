<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → D codegen backend

`zerodds-idlc --d` emits **native D** from OMG IDL: a self-contained source file
with a shared XCDR2 `Writer` (byte-identical to the `endpoints/d` core) and, per
IDL `struct`, a D struct plus a `ubyte[] marshalXCDR(Endian endian)` method whose
wire output matches the Rust goldens exactly.

Crate: [`crates/idl-d`](../crates/idl-d) · registered in `tools/idlc`
(`Backend::D`, flag `--d`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --d      # emits types.d
```

## Mapping

| IDL | D |
|-----|-----|
| `struct` `@final` | struct + compact `marshalXCDR` (no DHEADER) |
| `struct` `@appendable` | struct + DHEADER-framed `marshalXCDR` |
| `octet` / `uint8` | `ubyte` |
| `short`..`long long` (signed) | `short`..`long` (`cast` to the wire) |
| unsigned 16/32/64 | `ushort` / `uint` / `ulong` |
| `float` / `double` | `float` / `double` |
| `boolean` / `char` | `bool` / `char` |
| `string` | `string` |
| `sequence<octet>` | `ubyte[]` |

Only `@mutable` unions and non-literal array/collection bounds currently raise
`IdlDError::Unsupported`; unions, nested-struct members, maps, `long double`,
`wchar`, and `wstring` are emitted.

## Byte-identity (CI job `idl-d`)

The `crates/idl-d` suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates D for the `@final` golden type,
compiles+runs it (`gdc`), and asserts its LE + BE wire equals `golden_le.bin` /
`golden_be.bin`. Toolchain: `gdc` from apt.
