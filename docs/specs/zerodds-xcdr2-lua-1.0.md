<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-lua` v1.0 — Lua XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md) /
[`-julia`](zerodds-xcdr2-julia-1.0.md): the Lua binding of the XCDR2 wire — what
`zerodds-idlc --lua` emits and what the native `endpoints/lua` SDK provides.

## §1 Motivation

OMG has no IDL-to-Lua mapping. ZeroDDS defines a pure-Lua XCDR2 wire-core
(`endpoints/lua`) and a codegen backend (`crates/idl-lua`) that emits, per IDL
`struct`, a `marshal_<name>` function whose bytes equal the Rust `zerodds-cdr`
output. Built on `string.pack`/`string.unpack` (Lua 5.3+) — no C FFI.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```lua
function marshal_Reading(v, endian)
  local w = Writer.new(endian)
  w:putU32(v.id)
  w:putF32(v.value)
  w:putString(v.label)
  return w:bytes()
end
```

## §3 Required API-Surface

`endpoints/lua/zerodds.lua` (module table `M`) MUST provide: `LE`, `BE` (the
`string.pack` prefixes `"<"`/`">"`); `Writer`
(`putU8/putU16/putU32/putU64/putF32/putString/putSeqU8`, `bytes`); `Reader`
(`getU8/getU16/getU32/getU64/getF32/getString/getSeqU8` — the byte-exact inverse,
`f32` via `string.unpack("<f")`, `u64` via `"I8"`). The reader is a mutable
0-based cursor. Generated per struct: `marshal_<name>(v, endian)`. Decode is a
`Reader` walk (§10). Generated decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-lua`)

Per IDL `struct`, `zerodds-idlc --lua` MUST emit a `marshal_<name>` function and
the self-contained `Writer` prelude. Extensibility drives framing (§6);
unsupported constructs raise `IdlLuaError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | Lua | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `boolean` | 1 byte |
| `octet`/`uint8` | `integer` | 1 byte |
| `char` | `integer` | 1 byte |
| `short`/`int16` | `integer` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `integer` | 2 bytes LE, align 2 |
| `long`/`int32` | `integer` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `integer` | 4 bytes LE, align 4 |
| `long long`/`int64` | `integer` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `integer` | 8 bytes LE, align 4 |
| `float` | `number` | 4 bytes IEEE-754 LE (`string.pack("<f")`) |
| `double` | `number` | 8 bytes IEEE-754 LE |
| `string` | `string` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `string` (bytes) | uint32 count + raw bytes |

The `string.pack` endian prefix is a parameter, so a big-endian target produces
the same wire.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER (`uint32` body length + body).
`@mutable` — EMHEADER; not yet emitted → `Unsupported` (§11). The hand-written
`endpoints/lua` types are `@final`.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing (MD5 of key members' XCDR2-BE) is
runtime-provided; per-struct `keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/lua/zerodds.lua` is the reference `Writer`/`Reader`. `idl-lua` embeds a
byte-identical `Writer` prelude per generated file. Both byte-identical to
`zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-lua/tests/golden.rs::byte_identity_vs_rust_goldens`
  (`lua5.4`) — CI job `idl-lua`.
- **Endpoint:** `endpoints/lua/test.lua` — CI job `endpoints-lua`.

## §10 Examples

- Sync: [`endpoints/lua/example_sync.lua`](../../endpoints/lua/example_sync.lua)
  — poll loop, full field decode.
- Async: [`endpoints/lua/example_async.lua`](../../endpoints/lua/example_async.lua)
  — `coroutine.wrap` producer (resume yields the next decoded sample).
- Quickstart: [`endpoints/lua/QUICKSTART.md`](../../endpoints/lua/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope, uniform across all 17 idlc backends: generated
decode, per-struct `keyHash` from `@key`, `@mutable` EMHEADER, and
`wchar`/`wstring`/`map`/array/nested-struct/union/`long double` (raise
`Unsupported`). See the coverage doc's decision records.
