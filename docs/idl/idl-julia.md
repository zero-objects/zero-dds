<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — IDL → Julia codegen backend

`zerodds-idlc --julia` emits **native Julia** from OMG IDL: a self-contained
source file with a `Writer` (byte-identical to the `endpoints/julia` core) and,
per IDL `struct`, a Julia struct plus a `marshal_xcdr(v, endian)::Vector{UInt8}`
function whose wire output matches the Rust goldens exactly.

Crate: [`crates/idl-julia`](../crates/idl-julia) · registered in `tools/idlc`
(`Backend::Julia`, flag `--julia`, part of `--all`).

## Usage

```sh
zerodds-idlc types.idl --julia    # emits types.jl
```

## Mapping

| IDL | Julia |
|-----|-------|
| `struct` `@final` | struct + compact `marshal_xcdr` (no DHEADER) |
| `struct` `@appendable` | struct + DHEADER-framed `marshal_xcdr` |
| `octet` / `uint8` | `UInt8` |
| `short`..`long long` (signed) | `Int16`..`Int64` (`reinterpret` to the wire) |
| unsigned 16/32/64 | `UInt16` / `UInt32` / `UInt64` |
| `float` / `double` | `Float32` / `Float64` (`reinterpret` to bits) |
| `boolean` / `char` | `Bool` / `Char` |
| `string` | `String` |
| `sequence<octet>` | `Vector{UInt8}` |

Only `@mutable` unions and non-literal array/collection bounds currently raise
`IdlJuliaError::Unsupported`; unions, nested-struct members, maps, `long double`,
`wchar`, and `wstring` are emitted.

## Byte-identity (CI job `idl-julia`)

The `crates/idl-julia` suite: string smoke tests always, plus
`byte_identity_vs_rust_goldens` — generates Julia for the `@final` golden type,
runs it (`julia`), and asserts its LE + BE wire equals `golden_le.bin` /
`golden_be.bin`. Toolchain: the official Julia 1.10 tarball; on hardened kernels
the bundled libs need `PT_GNU_STACK`'s exec bit cleared (`clear_execstack.py`).
