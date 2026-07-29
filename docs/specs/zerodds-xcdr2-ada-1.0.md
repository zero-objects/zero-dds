<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-ada` v1.0 — Ada XCDR2 TypeSupport-Codegen

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md) /
[`-julia`](zerodds-xcdr2-julia-1.0.md): the Ada binding of the XCDR2 wire — what
`zerodds-idlc --ada` emits and what the native `endpoints/ada` SDK provides.

## §1 Motivation

OMG has no IDL-to-Ada mapping. ZeroDDS defines an Ada XCDR2 binding in two layers:
the codegen backend (`crates/idl-ada`) emits a pure-Ada package (`Zdgen`) with a
bounded XCDR2 wire buffer and, per IDL `struct`, a `Marshal` function; the native
endpoint (`endpoints/ada`, ADR 0013 Stage 1) binds the audited C89 wire-core
(`endpoints/c`) through `Interfaces.C`. Both are byte-identical to `zerodds-cdr`.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }` the
`idl-ada` backend emits into package `Zdgen`:

```ada
type Reading is record
   id    : Interfaces.Unsigned_32;
   value : Interfaces.IEEE_Float_32;
   label : Ada.Strings.Unbounded.Unbounded_String;
end record;

function Marshal (V : Reading; Endian : Endianness) return Byte_Array is
   W : Buf_T;
begin
   Put_U32 (W, V.id);
   Put_F32 (W, V.value);
   Put_String (W, To_String (V.label));
   return Bytes (W);
end Marshal;
```

## §3 Required API-Surface

`endpoints/ada/src/zdw.ads` (package `Zdw`) MUST provide the wire cursors and
primitives bound over the C core: `Writer_Init`/`Reader_Init`; writer
`Put_U8/Put_U16/Put_U32/Put_U64/Put_Bool/Put_F32/Put_F64/Put_String/Put_Seq_U8`;
reader `Get_U8/Get_U16/Get_U32/Get_U64/Get_F32/Get_String/Get_Seq_U8` — the
byte-exact inverse (`f32` via the C `zdw_get_f32`, `u64` via the `Zdw_U64` two-half
record). Decode is a `Zdw_Reader` walk (§10). The `idl-ada` codegen emits, per
struct, an Ada record + a `Marshal` function. Generated decode / key hash — §11.

## §4 Codegen-Pflicht (`idl-ada`)

Per IDL `struct`, `zerodds-idlc --ada` MUST emit package `Zdgen` (spec + body): an
Ada record, a `Marshal` function, and the self-contained bounded XCDR2 wire
helpers (`Buf_T`, `Put_*`). Extensibility drives framing (§6); unsupported
constructs raise `IdlAdaError::Unsupported` (§11).

## §5 Wire-Type-Mapping

| IDL | Ada (`idl-ada` / `endpoints/ada`) | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `Boolean` | 1 byte |
| `octet`/`uint8` | `Unsigned_8` / `unsigned_char` | 1 byte |
| `char` | `Character` / `unsigned_char` | 1 byte |
| `short`/`int16` | `Integer_16` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `Unsigned_16` / `unsigned` | 2 bytes LE, align 2 |
| `long`/`int32` | `Integer_32` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `Unsigned_32` / `unsigned_long` | 4 bytes LE, align 4 |
| `long long`/`int64` | `Integer_64` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `Unsigned_64` / `Zdw_U64` | 8 bytes LE, align 4 |
| `float` | `IEEE_Float_32` / `C_float` | 4 bytes IEEE-754 LE |
| `double` | `IEEE_Float_64` / `double` | 8 bytes IEEE-754 LE |
| `string` | `Unbounded_String` / `char_array` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `Unbounded_String` / `Byte_Array` | uint32 count + raw bytes |

Byte order is an explicit parameter (`Endianness` / the `ZDW_LE`/`ZDW_BE` flag), so
a big-endian target produces the same wire.

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER (`uint32` body length + body).
`@mutable` — EMHEADER; not yet emitted → `Unsupported` (§11). The hand-written
`endpoints/ada` `Sample_Sensor` type is `@final`.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing (MD5 of key members' XCDR2-BE) is
runtime-provided; per-struct `keyHash` codegen — §11.

## §8 Wire-Core

`endpoints/ada/src/zdw.ads` binds the reference C89 wire-core (`endpoints/c`);
`idl-ada` embeds a byte-identical bounded `Buf_T`/`Put_*` per generated file. Both
byte-identical to `zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Codegen:** `crates/idl-ada/tests/golden.rs::byte_identity_vs_rust_goldens`
  (`gnatmake`) — CI job `idl-ada`.
- **Endpoint:** `endpoints/ada/test/test_byte_identity.adb` (+ `test_udp_loopback`)
  — CI job `endpoints-ada`.

## §10 Examples

- Sync: [`endpoints/ada/test/example_sync.adb`](../../endpoints/ada/test/example_sync.adb)
  — poll loop (`Mailbox.Try_Receive`), full field decode.
- Async: [`endpoints/ada/test/example_async.adb`](../../endpoints/ada/test/example_async.adb)
  — a `Reader_Task` + protected `Mailbox` (the main blocks on `Inbox.Receive`).
- Quickstart: [`endpoints/ada/QUICKSTART.md`](../../endpoints/ada/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope, uniform across all 17 idlc backends: generated
decode, per-struct `keyHash` from `@key`, `@mutable` EMHEADER, and
`wchar`/`wstring`/`map`/array/nested-struct/union/`long double` (raise
`Unsupported`). See the coverage doc's decision records.
