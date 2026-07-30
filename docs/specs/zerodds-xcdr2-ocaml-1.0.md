<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-ocaml` v1.0 — OCaml XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md) /
[`-nim`](zerodds-xcdr2-nim-1.0.md): the OCaml binding of the XCDR2 wire — what
`zerodds-idlc --ocaml` emits and what the native `endpoints/ocaml` SDK provides.

## §1 Motivation

OMG has no IDL-to-OCaml mapping. ZeroDDS defines a pure-OCaml XCDR2 wire-core
(`endpoints/ocaml`) and a codegen backend (`crates/idl-ocaml`) that emits, per IDL
`struct`, an OCaml `module` with a record type `t` plus a `marshal` function whose
bytes equal the Rust `zerodds-cdr` output. No C stubs, no Ctypes: OCaml `Buffer`
and `Bytes` carry the wire.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```ocaml
module Reading = struct
  type t = { id : int; value : float; label : string }

  let marshal (v : t) (endian : Wire.endian) : bytes =
    let open Wire in
    let w = writer endian in
    put_u32 w v.id;
    put_f32 w v.value;
    put_string w v.label;
    bytes w
end
```

Each IDL struct is its own module, so field labels never collide across types.

## §3 Required API-Surface

`endpoints/ocaml/zerodds.ml` (module `Zerodds`) MUST provide: `Wire.endian`
(`LE`, `BE`); `Wire` writer
(`put_u8/put_u16/put_u32/put_u64/put_f32/put_string/put_seq_u8`, `bytes`); `Wire`
reader (`get_u8/get_u16/get_u32/get_u64/get_f32/get_string/get_seq_u8` — the
byte-exact inverse, `f32` via `Int32.float_of_bits`, `u64` via `Int64`). The
reader is a mutable-position cursor. Generated per struct: `marshal (endian) :
bytes`. Decode is a `Wire` reader walk (§10). Generated decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-ocaml`)

Per IDL `struct`, `zerodds-idlc --ocaml` MUST emit an OCaml `module` with a record
type `t`, a `marshal (v : t) (endian : Wire.endian) : bytes` function, and the
self-contained `Wire` module. Extensibility drives framing (§6); unsupported
constructs raise `IdlOcamlError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | OCaml | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `bool` | 1 byte |
| `octet`/`uint8` | `int` | 1 byte |
| `char` | `char` | 1 byte |
| `short`/`int16` | `int` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `int` | 2 bytes LE, align 2 |
| `long`/`int32` | `int` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `int` | 4 bytes LE, align 4 |
| `long long`/`int64` | `int64` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `int64` | 8 bytes LE, align 4 |
| `float` | `float` | 4 bytes IEEE-754 LE (`Int32.bits_of_float`) |
| `double` | `float` | 8 bytes IEEE-754 LE (`Int64.bits_of_float`) |
| `string` | `string` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `bytes` | uint32 count + raw bytes |

Byte order is an explicit parameter, so a big-endian target produces the same wire.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER (`uint32` body length + body).
`@mutable` structs — DHEADER + per-member EMHEADER (LC4) list. `@mutable` unions are not yet emitted → `Unsupported` (§11). The hand-written
`endpoints/ocaml` types are `@final`.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing (MD5 of key members' XCDR2-BE) is
runtime-provided; per-struct `keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/ocaml/zerodds.ml` is the reference `Wire` writer/reader. `idl-ocaml`
embeds a byte-identical `Wire` module per generated file. Both byte-identical to
`zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-ocaml/tests/golden.rs::byte_identity_vs_rust_goldens`
  (`ocamlfind ocamlopt`) — CI job `idl-ocaml`.
- **Endpoint:** `endpoints/ocaml/test.ml` — CI job `endpoints-ocaml`.

## §10 Examples

- Sync: [`endpoints/ocaml/example_sync.ml`](../../endpoints/ocaml/example_sync.ml)
  — poll loop, full field decode.
- Async: [`endpoints/ocaml/example_async.ml`](../../endpoints/ocaml/example_async.ml)
  — `AsyncReader` Thread + Mutex/Condition mailbox (`recv`).
- Quickstart: [`endpoints/ocaml/QUICKSTART.md`](../../endpoints/ocaml/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope: generated decode and per-struct `keyHash` from
`@key`. `@mutable` unions and non-literal bounds also raise `Unsupported`;
`@mutable` structs, unions, `wchar`, `wstring`, `map`, arrays, nested-struct
members, and `long double` are emitted. See the coverage doc's decision records.
