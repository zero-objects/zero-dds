<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → Zig codegen backend

`zerodds-idlc --zig` emits **native Zig** from OMG IDL: a self-contained source
file with a shared XCDR2 `Writer` (byte-identical to the `endpoints/zig` core)
and, per IDL `struct`, a Zig struct plus a `marshalXCDR(endian, allocator) ![]u8`
method whose wire output matches the Rust goldens exactly.

Crate: [`crates/idl-zig`](../crates/idl-zig) · registered in `tools/idlc`
(`Backend::Zig`, flag `--zig`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --zig      # emits types.zig
```

## Mapping

| IDL | Zig |
|-----|-----|
| `struct` `@final` | struct + compact `marshalXCDR` (no DHEADER) |
| `struct` `@appendable` | struct + DHEADER-framed `marshalXCDR` |
| `octet` / `uint8` | `u8` |
| `short`..`long long` (signed) | `i16`..`i64` (`@bitCast` to the wire) |
| unsigned 16/32/64 | `u16` / `u32` / `u64` |
| `float` / `double` | `f32` / `f64` |
| `boolean` / `char` | `bool` / `u8` |
| `string` / `sequence<octet>` | `[]const u8` |

`@mutable`, unions, nested-struct members, maps, `long double`, `wchar`, and
`wstring` currently raise `IdlZigError::Unsupported`.

## Byte-identity (CI job `idl-zig`)

The `crates/idl-zig` suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates Zig for the `@final` golden type,
compiles+runs it (`zig run`), and asserts its LE + BE wire equals
`golden_le.bin` / `golden_be.bin`. Toolchain: the official Zig 0.13 tarball.
