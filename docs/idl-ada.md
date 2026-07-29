<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → Ada codegen backend

`zerodds-idlc --ada` emits **native Ada 2012** from OMG IDL: a self-contained
package (spec `.ads` + body `.adb`) with a bounded XCDR2 wire buffer
(byte-identical to the `endpoints/ada-native` core) and, per IDL `struct`, an
Ada record plus a `Marshal (V; Endian) return Byte_Array` function whose wire
output matches the Rust goldens exactly.

Crate: [`crates/idl-ada`](../crates/idl-ada) · registered in `tools/idlc`
(`Backend::Ada`, flag `--ada`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --ada      # emits types.ads + types.adb
```

## Mapping

| IDL | Ada |
|-----|-----|
| `struct` `@final` | record + compact `Marshal` (no DHEADER) |
| `struct` `@appendable` | record + DHEADER-framed `Marshal` |
| `octet` / `uint8` | `Interfaces.Unsigned_8` |
| `short`..`long long` (signed) | `Integer_16`..`Integer_64` (`'Mod` to the wire) |
| unsigned 16/32/64 | `Unsigned_16` / `Unsigned_32` / `Unsigned_64` |
| `float` / `double` | `IEEE_Float_32` / `IEEE_Float_64` |
| `boolean` / `char` | `Boolean` / `Character` |
| `string` | `Ada.Strings.Unbounded.Unbounded_String` |
| `sequence<octet>` | `Unbounded_String` (raw bytes) |

`@mutable`, unions, nested-struct members, maps, `long double`, `wchar`, and
`wstring` currently raise `IdlAdaError::Unsupported`.

## Byte-identity (CI job `idl-ada`)

The `crates/idl-ada` suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates the Ada package for the `@final`
golden type, compiles+runs it (`gnatmake`), and asserts its LE + BE wire equals
`golden_le.bin` / `golden_be.bin`. Toolchain: `gnat` + `gprbuild` from apt.
