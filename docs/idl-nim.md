<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → Nim codegen backend

`zerodds-idlc --nim` emits **native Nim** from OMG IDL: a self-contained source
file with a shared XCDR2 `Writer` (byte-identical to the `endpoints/nim` core)
and, per IDL `struct`, a Nim `object` plus a `marshalXCDR(endian): seq[byte]`
proc whose wire output matches the Rust goldens exactly.

Crate: [`crates/idl-nim`](../crates/idl-nim) · registered in `tools/idlc`
(`Backend::Nim`, flag `--nim`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --nim      # emits types.nim
```

## Mapping

| IDL | Nim |
|-----|-----|
| `struct` `@final` | object + compact `marshalXCDR` (no DHEADER) |
| `struct` `@appendable` | object + DHEADER-framed `marshalXCDR` |
| `octet` / `uint8` | `uint8` |
| `short`..`long long` (signed) | `int16`..`int64` (`cast` to the wire) |
| unsigned 16/32/64 | `uint16` / `uint32` / `uint64` |
| `float` / `double` | `float32` / `float64` |
| `boolean` / `char` | `bool` / `char` |
| `string` | `string` |
| `sequence<octet>` | `seq[byte]` |

`@mutable`, unions, nested-struct members, maps, `long double`, `wchar`, and
`wstring` currently raise `IdlNimError::Unsupported`.

## Byte-identity (CI job `idl-nim`)

The `crates/idl-nim` suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates Nim for the `@final` golden type,
compiles+runs it (`nim c -r`), and asserts its LE + BE wire equals
`golden_le.bin` / `golden_be.bin`. Toolchain: the official Nim 2.0 tarball.
