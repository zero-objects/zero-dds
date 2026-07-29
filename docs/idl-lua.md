<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → Lua codegen backend

`zerodds-idlc --lua` emits **native pure-Lua** from OMG IDL: a self-contained
source file with a `Writer` built on `string.pack` (byte-identical to the
`endpoints/lua` core, **no FFI**) and, per IDL `struct`, a
`marshal_<Name>(v, endian)` function whose wire output matches the Rust goldens
exactly. `v` is a plain Lua table of the struct's fields.

Crate: [`crates/idl-lua`](../crates/idl-lua) · registered in `tools/idlc`
(`Backend::Lua`, flag `--lua`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --lua      # emits types.lua
```

## Mapping

| IDL | Lua |
|-----|-----|
| `struct` `@final` | `marshal_<Name>` — compact (no DHEADER) |
| `struct` `@appendable` | `marshal_<Name>` — DHEADER-framed |
| integers | `string.pack(endian .. "I2/I4/I8", v)` (signed masked to wire) |
| `float` / `double` | `string.pack(endian .. "f"/"d", v)` |
| `boolean` / `octet` / `char` | one octet |
| `string` | CDR string (u32 len + bytes + NUL) |
| `sequence<octet>` | a Lua string (u32 len + bytes) |

`@mutable`, unions, nested-struct members, maps, `long double`, `wchar`, and
`wstring` currently raise `IdlLuaError::Unsupported`.

## Byte-identity (CI job `idl-lua`)

The `crates/idl-lua` suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates Lua for the `@final` golden type,
runs it (`lua5.4`), and asserts its LE + BE wire equals `golden_le.bin` /
`golden_be.bin`. Toolchain: `lua5.4` from apt (nothing else — pure Lua).
