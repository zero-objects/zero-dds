<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → Elixir codegen backend

`zerodds-idlc --elixir` emits **native Elixir** from OMG IDL: a self-contained
source file with a `<Pkg>.Wire` module (byte-identical to the `endpoints/elixir`
core, built on Elixir bitstrings) and, per IDL `struct`, a `<Pkg>.<Name>` module
with `defstruct` and a `marshal_xcdr(v, endian)` function whose wire output
matches the Rust goldens exactly.

Crate: [`crates/idl-elixir`](../crates/idl-elixir) · registered in `tools/idlc`
(`Backend::Elixir`, flag `--elixir`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --elixir    # emits types.ex
```

## Mapping

| IDL | Elixir |
|-----|--------|
| `struct` `@final` | module + `defstruct` + compact `marshal_xcdr` (no DHEADER) |
| `struct` `@appendable` | module + DHEADER-framed `marshal_xcdr` |
| integers (signed/unsigned) | bitstrings (`<<v::little-N>>`, 2's complement) |
| `float` / `double` | `<<v::float-little-32/64>>` |
| `boolean` / `octet` / `char` | one octet |
| `string` | CDR string (u32 len + bytes + NUL) |
| `sequence<octet>` | a binary (u32 len + bytes) |

`@mutable`, unions, nested-struct members, maps, `long double`, `wchar`, and
`wstring` currently raise `IdlElixirError::Unsupported`.

## Byte-identity (CI job `idl-elixir`)

The `crates/idl-elixir` suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates Elixir for the `@final` golden type,
runs it (`elixir`), and asserts its LE + BE wire equals `golden_le.bin` /
`golden_be.bin`. Toolchain: `erlang` + `elixir` from apt.
