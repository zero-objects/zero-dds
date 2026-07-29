<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-elixir` v1.0 — Elixir XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-nim`](zerodds-xcdr2-nim-1.0.md):
the Elixir/BEAM binding of the XCDR2 wire — what `zerodds-idlc --elixir` emits and
what the native `endpoints/elixir` SDK provides.

## §1 Motivation

OMG has no IDL-to-Elixir mapping. ZeroDDS provides a pure-Elixir XCDR2 wire-core
(`endpoints/elixir`, built on bitstrings) and a codegen backend
(`crates/idl-elixir`) that emits, per IDL `struct`, an Elixir module with
`defstruct` plus a `marshal_xcdr` function whose bytes equal the Rust
`zerodds-cdr` output.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```elixir
defmodule Reading do
  defstruct [:id, :value, :label]

  def marshal_xcdr(%__MODULE__{} = v, endian) do
    ZeroDDS.Wire.writer(endian)
    |> ZeroDDS.Wire.put_u32(v.id)
    |> ZeroDDS.Wire.put_f32(v.value)
    |> ZeroDDS.Wire.put_string(v.label)
    |> ZeroDDS.Wire.bytes()
  end
end
```

## §3 Required API-Surface

`endpoints/elixir/lib/zerodds.ex` (`ZeroDDS.Wire`) MUST provide: `writer(endian)`;
`put_u8/put_u16/put_u32/put_u64/put_f32/put_bytes/put_string/put_seq_u8`,
`bytes`; `reader(bin, endian)`; `get_u8/get_u16/get_u32/get_u64/get_f32/
get_string/get_seq_u8` (the byte-exact inverse via bitstring pattern matching,
each returning `{value, reader}`). Decode is a `Reader` walk (§10); generated
decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-elixir`)

Per IDL `struct`, `zerodds-idlc --elixir` MUST emit a `<Pkg>.<Name>` module with
`defstruct` and `marshal_xcdr(v, endian)`, plus the self-contained `<Pkg>.Wire`
module. Extensibility drives framing (§6); unsupported constructs raise
`IdlElixirError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | Elixir | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `boolean` | 1 byte |
| `octet`/`uint8` | `integer` | 1 byte |
| `char` | `integer` | 1 byte |
| `short`..`long long` (signed) | `integer` | 2/4/8 bytes LE (`<<v::little-N>>`, 2's complement) |
| unsigned 16/32/64 | `integer` | 2/4/8 bytes LE |
| `float` | `float` | 4 bytes IEEE-754 LE (`<<v::float-little-32>>`) |
| `double` | `float` | 8 bytes IEEE-754 LE |
| `string` | `binary` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `binary` | uint32 count + raw bytes |

Elixir bitstrings encode signed and unsigned identically in 2's complement.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER (`uint32` body length + body).
`@mutable` — EMHEADER; not yet emitted → `Unsupported` (§11).

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing is runtime-provided; per-struct
`keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/elixir/lib/zerodds.ex` (`ZeroDDS.Wire`) is the reference writer/reader.
`idl-elixir` embeds a byte-identical `<Pkg>.Wire` per generated file. Both
byte-identical to `zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-elixir/tests/golden.rs::byte_identity_vs_rust_goldens`
  (`elixir`) — CI job `idl-elixir`.
- **Endpoint:** `endpoints/elixir/test.exs` — CI job `endpoints-elixir`.

## §10 Examples

- Sync: [`endpoints/elixir/example_sync.exs`](../../endpoints/elixir/example_sync.exs)
  — poll loop, full field decode.
- Async: [`endpoints/elixir/example_async.exs`](../../endpoints/elixir/example_async.exs)
  — a process/mailbox (`AsyncReader` sends `{:zerodds_sample, body}`; `receive`).
- Quickstart: [`endpoints/elixir/QUICKSTART.md`](../../endpoints/elixir/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope, uniform across all 17 idlc backends: generated
decode, per-struct `keyHash` from `@key`, `@mutable` EMHEADER, and
`wchar`/`wstring`/`map`/array/nested-struct/union/`long double` (raise
`Unsupported`). See the coverage doc's decision records.
