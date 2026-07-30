<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-julia` v1.0 — Julia XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md) /
[`-nim`](zerodds-xcdr2-nim-1.0.md): the Julia binding of the XCDR2 wire — what
`zerodds-idlc --julia` emits and what the native `endpoints/julia` SDK provides.

## §1 Motivation

OMG has no IDL-to-Julia mapping. ZeroDDS defines a pure-Julia XCDR2 wire-core
(`endpoints/julia`) and a codegen backend (`crates/idl-julia`) that emits, per IDL
`struct`, a Julia `struct` plus a `marshal_xcdr` function whose bytes equal the
Rust `zerodds-cdr` output. No C stubs, no ccall: Julia `Vector{UInt8}` carries the
wire.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```julia
struct Reading
    id::UInt32
    value::Float32
    label::String
end

function marshal_xcdr(v::Reading, endian::Endian)::Vector{UInt8}
    w = Writer(endian)
    put_u32!(w, v.id)
    put_f32!(w, v.value)
    put_string!(w, v.label)
    bytes(w)
end
```

## §3 Required API-Surface

`endpoints/julia/zerodds.jl` (module `ZeroDDS`) MUST provide: `Endian` (`LE`,
`BE`); `Writer` (`put_u8!/put_u16!/put_u32!/put_u64!/put_f32!/put_bytes!/
put_string!/put_seq_u8!`, `bytes`); `Reader` (`get_u8/get_u16/get_u32/get_u64/
get_f32/get_string/get_seq_u8` — the byte-exact inverse, `f32` via
`reinterpret(Float32, u32)`, `u64` via `UInt64`). The reader is a mutable
0-based cursor. Generated per struct: `marshal_xcdr(v, endian)::Vector{UInt8}`.
Decode is a `Reader` walk (§10). Generated decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-julia`)

Per IDL `struct`, `zerodds-idlc --julia` MUST emit a Julia `struct`, a
`marshal_xcdr` function, and the self-contained `Writer`. Extensibility drives
framing (§6); unsupported constructs raise `IdlJuliaError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | Julia | Wire (XCDR2, align cap 4) |
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
| `float` | `Float32` | 4 bytes IEEE-754 LE (`reinterpret(UInt32, ·)`) |
| `double` | `Float64` | 8 bytes IEEE-754 LE |
| `string` | `String` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `Vector{UInt8}` | uint32 count + raw bytes |

Byte order is an explicit parameter, so a big-endian target produces the same wire.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER (`uint32` body length + body).
`@mutable` structs — DHEADER + per-member EMHEADER (LC4) list. `@mutable` unions are not yet emitted → `Unsupported` (§11). The hand-written
`endpoints/julia` types are `@final`.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing (MD5 of key members' XCDR2-BE) is
runtime-provided; per-struct `keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/julia/zerodds.jl` is the reference `Writer`/`Reader`. `idl-julia`
embeds a byte-identical `Writer` per generated file. Both byte-identical to
`zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-julia/tests/golden.rs::byte_identity_vs_rust_goldens`
  (`julia`) — CI job `idl-julia`.
- **Endpoint:** `endpoints/julia/test.jl` — CI job `endpoints-julia`.

## §10 Examples

- Sync: [`endpoints/julia/example_sync.jl`](../../endpoints/julia/example_sync.jl)
  — poll loop, full field decode.
- Async: [`endpoints/julia/example_async.jl`](../../endpoints/julia/example_async.jl)
  — `Task` + `Channel` (`take!` via `recv`).
- Quickstart: [`endpoints/julia/QUICKSTART.md`](../../endpoints/julia/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope: generated decode and per-struct `keyHash` from
`@key`. `@mutable` unions and non-literal bounds also raise `Unsupported`;
`@mutable` structs, unions, `wchar`, `wstring`, `map`, arrays, nested-struct
members, and `long double` are emitted. See the coverage doc's decision records.
