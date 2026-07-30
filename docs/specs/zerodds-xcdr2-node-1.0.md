<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-xcdr2-node` v1.0 — Node (JavaScript) XCDR2 TypeSupport

**Status:** normative · **Wire:** XCDR2 (PLAIN_CDR2), byte-identical to `zerodds-cdr`.

Analogous to [`-ts`](zerodds-xcdr2-ts-1.0.md) / [`-go`](zerodds-xcdr2-go-1.0.md):
the Node (pure JavaScript) binding of the XCDR2 wire — the native
`endpoints/node` SDK, and how IDL types reach Node.

## §1 Motivation

OMG has no IDL-to-JS mapping. ZeroDDS provides a pure-JS XCDR2 wire-core
(`endpoints/node`), no deps, byte-identical to the Rust core.

## §2 Marshal-Pattern

Per IDL `@final struct Reading { uint32 id; float value; string label; }`:

```js
class Reading {
  constructor(id, value, label) { this.id = id; this.value = value; this.label = label; }
  marshal(endian) {
    const w = new Writer(endian);
    w.putU32(this.id);
    w.putF32(this.value);
    w.putString(this.label);
    return w.bytes();
  }
}
```

## §3 Required API-Surface

`endpoints/node/zerodds.js` MUST export: `LITTLE`/`BIG`; `Writer`
(`putU8/putU16/putU32/putU64(BigInt)/putF32/putBytes/putString/putSeqU8`,
`bytes` → Buffer); `Reader` (`getU8/getU16/getU32/getU64(BigInt)/getF32/
getString/getSeqU8` — the byte-exact inverse). `f32` via `Buffer.writeFloatLE`/
`readFloatLE`; `u64` via `BigInt`. Decode is a `Reader` walk (§10); generated
decode / key hash — §11.

## §4 Codegen (via `idl-ts`)

Node has no dedicated JS idl backend: IDL types are generated as TypeScript with
`zerodds-idlc --ts` (`crates/idl-ts`, spec `zerodds-xcdr2-ts`) and run on Node —
the TS TypeSupport's `encode`/`decode` produce the same wire. The pure-JS
`endpoints/node` wire-core is byte-identical, so hand-written JS types (§2) and
idl-ts-generated types share the same wire.

## §5 Wire-Type-Mapping

| IDL | JS | Wire (XCDR2, align cap 4) |
|-----|-----|-----|
| `boolean` | `boolean` | 1 byte |
| `octet`/`uint8` | `number` | 1 byte |
| `char` | `number` | 1 byte |
| `short`/`int16` | `number` | 2 bytes LE, align 2 |
| `unsigned short`/`uint16` | `number` | 2 bytes LE, align 2 |
| `long`/`int32` | `number` | 4 bytes LE, align 4 |
| `unsigned long`/`uint32` | `number` | 4 bytes LE, align 4 |
| `long long`/`int64` | `bigint` | 8 bytes LE, align 4 |
| `unsigned long long`/`uint64` | `bigint` | 8 bytes LE, align 4 |
| `float` | `number` | 4 bytes IEEE-754 LE (`writeFloatLE`) |
| `double` | `number` | 8 bytes IEEE-754 LE |
| `string` | `string` | uint32 (len+1) + UTF-8 + NUL |
| `sequence<octet>` | `Buffer` | uint32 count + raw bytes |

`bigint` for 64-bit integers is mandatory (a `number` loses precision above 2^53).

## §6 Extensibility

`@final` — compact. `@appendable` — DHEADER. `@mutable` — EMHEADER; via idl-ts.
The hand-written `endpoints/node` types are `@final`.

## §7 Key-Extraction

Non-keyed → 16 zero bytes. Keyed key-hashing is runtime/idl-ts-provided.

## §8 Wire-Core

`endpoints/node/zerodds.js` is the reference `Writer`/`Reader`, byte-identical to
`zerodds-cdr`.

## §9 Conformance

Conformant iff the `@final` golden encoding equals `golden_le.bin` /
`golden_be.bin` byte-for-byte.

- **Endpoint:** `endpoints/node/test.js` (`node --test`) — CI job `endpoints-node`.
- **Codegen:** inherited from `crates/idl-ts/tests` (spec `zerodds-xcdr2-ts`).

## §10 Examples

- Sync: [`endpoints/node/example_sync.js`](../../endpoints/node/example_sync.js)
  — poll loop, full field decode.
- Async: [`endpoints/node/example_async.js`](../../endpoints/node/example_async.js)
  — `for await (const body of reader.stream())` async iterator.
- Quickstart: [`endpoints/node/QUICKSTART.md`](../../endpoints/node/QUICKSTART.md).

## §11 Errata + Open-Questions

Consciously out of v1.0 scope for the hand-written wire-core, uniform across all
endpoints: generated decode, per-struct `keyHash`, and `@mutable`/`wchar`/
`wstring`/`map`/array/nested/union — provided via the idl-ts codegen path where
needed. See the coverage doc's decision records.
