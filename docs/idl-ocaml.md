<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → OCaml codegen backend

`zerodds-idlc --ocaml` emits **native OCaml** from OMG IDL: a self-contained
source file with a `Wire` module (byte-identical to the `endpoints/ocaml` core,
built on `Buffer`/`Bytes`) and, per IDL `struct`, a module with a record type
`t` and a `marshal(v, endian): bytes` function whose wire output matches the
Rust goldens exactly.

Crate: [`crates/idl-ocaml`](../crates/idl-ocaml) · registered in `tools/idlc`
(`Backend::OCaml`, flag `--ocaml`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --ocaml    # emits types.ml
```

## Mapping

| IDL | OCaml |
|-----|-------|
| `struct` `@final` | `module <Name>` with record `t` + compact `marshal` |
| `struct` `@appendable` | record `t` + DHEADER-framed `marshal` |
| 8/16/32-bit integers | `int` (`Wire.put_u8/16/32`) |
| 64-bit integers | `int64` (`Wire.put_u64`) |
| `float` / `double` | `float` (`Int32.bits_of_float` / `Int64.bits_of_float`) |
| `boolean` / `char` | `bool` / `char` |
| `string` | `string` |
| `sequence<octet>` | `bytes` |

Each struct is its own module, so record field names never clash across types.
`@mutable`, unions, nested-struct members, maps, `long double`, `wchar`, and
`wstring` currently raise `IdlOcamlError::Unsupported`.

## Byte-identity (CI job `idl-ocaml`)

The `crates/idl-ocaml` suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates OCaml for the `@final` golden type,
compiles+runs it (`ocamlfind ocamlopt`), and asserts its LE + BE wire equals
`golden_le.bin` / `golden_be.bin`. Toolchain: `ocaml-nox` + `ocaml-findlib`.
