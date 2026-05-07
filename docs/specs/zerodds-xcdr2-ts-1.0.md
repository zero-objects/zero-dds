# `zerodds-xcdr2-ts` v1.0 — TypeScript TypeSupport-Codegen

ZeroDDS Vendor-Spec. Implementiert in `crates/idl-ts` (Codegen) und
`crates/ts-node/src/cdr/` + `crates/ts-wasm/src/cdr/` (Helper-Module
`@zerodds/cdr`). Konformanz gegen
[`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md).

## §1 Motivation

OMG **DDS-TS 1.0** spezifiziert TypeScript-Mapping fuer IDL-Datentypen,
sagt aber **nichts** ueber Wire-Encoding. Heute liefert `crates/idl-ts`
nur Type-Definitions; `encode/decode` fehlen komplett.

Diese Spec schliesst die Luecke fuer Browser- und Node-Targets
gleichermassen.

## §2 TypeSupport-Pattern

TypeScript-idiomatisch via Interface + Module-Level-Const-Object:

```ts
// @zerodds/cdr
export type ExtensibilityKind = 'final' | 'appendable' | 'mutable';
export type EndianMode = 'le' | 'be';

export interface DdsTopicType<T> {
    readonly typeName: string;
    readonly isKeyed: boolean;
    readonly extensibility: ExtensibilityKind;

    encode(sample: T, endian?: EndianMode): Uint8Array;
    decode(bytes: Uint8Array, offset?: number, length?: number): T;
    keyHash(sample: T): Uint8Array;  // 16 Bytes
}
```

Generierter Code: pro IDL-`struct` ein **Module-Level `const`** das
`DdsTopicType<T>` implementiert.

## §3 Required API-Surface

```ts
import { DdsTopicType, Xcdr2Writer, Xcdr2Reader, md5 } from '@zerodds/cdr';

export interface MyType {
    x: number;
    y: number;
}

export const MyTypeTypeSupport: DdsTopicType<MyType> = {
    typeName: 'MyType',
    isKeyed: false,
    extensibility: 'final',

    encode(s: MyType, endian: EndianMode = 'le'): Uint8Array {
        const w = new Xcdr2Writer(endian);
        w.writeInt32(s.x);
        w.writeInt32(s.y);
        return w.toBytes();
    },
    decode(bytes: Uint8Array, offset = 0, length = bytes.length - offset): MyType {
        const r = new Xcdr2Reader(bytes, offset, length, 'le');
        return { x: r.readInt32(), y: r.readInt32() };
    },
    keyHash(_s: MyType): Uint8Array {
        return new Uint8Array(16);
    },
};
```

## §4 Codegen-Pflicht (idl-ts)

Pro IDL-`struct` MUSS `idl-ts` emittieren:

1. `interface MyType { ... }` (existiert).
2. **NEU:** `export const MyTypeTypeSupport: DdsTopicType<MyType>`.
3. **NEU:** Auto-Import-Stub: jeder generierte File hat
   `import { DdsTopicType, Xcdr2Writer, Xcdr2Reader } from '@zerodds/cdr';`.

Generierter Code lebt in einem TS-Namespace der dem IDL-Modul-Pfad
entspricht (z.B. `module Outer.Inner { struct S }` → `export
namespace Outer.Inner { export interface S { ... }; export const
STypeSupport: DdsTopicType<S> }`).

## §5 Wire-Type-Mapping

| IDL | TypeScript | Wire (XCDR2 LE) |
|-----|-----------|-----------------|
| `boolean` | `boolean` | 1 Byte |
| `octet` | `number` (0-255) | 1 Byte |
| `char` | `string` (1 char) | 1 Byte |
| `wchar` | `string` (1 char) | 2 Byte LE |
| `short` / `int16` | `number` | 2 Byte LE Align(2) |
| `unsigned short` / `uint16` | `number` | 2 Byte LE Align(2) |
| `long` / `int32` | `number` | 4 Byte LE Align(4) |
| `unsigned long` / `uint32` | `number` | 4 Byte LE Align(4) |
| `long long` / `int64` | `bigint` | 8 Byte LE Align(8) |
| `unsigned long long` / `uint64` | `bigint` | 8 Byte LE Align(8) |
| `float` | `number` | 4 Byte IEEE-754 LE |
| `double` | `number` | 8 Byte IEEE-754 LE |
| `string` | `string` | uint32 length+1 + UTF-8 + NUL |
| `wstring` | `string` | uint32 length + UTF-16-LE |
| `sequence<T>` | `T[]` | uint32 count + T[] |
| `T[N]` | `T[]` (length-checked) | T[] N Elemente |
| nested `struct U` | `U` | inline `UTypeSupport.encode(...)` |
| `enum E` | `enum E { A, B }` | int32 LE |
| `@optional T` | `T \| null` | M-Flag / present-byte |

`bigint` fuer 64-Bit-Integer ist Pflicht (Number verliert Praezision
ab 2^53). Codegen emittiert explizit `bigint` fuer `int64`/`uint64`.

## §6 Extensibility

```ts
extensibility: 'final',     // kein DHEADER
extensibility: 'appendable', // 4-Byte uint32 DHEADER
extensibility: 'mutable',    // PL_CDR2 mit EMHEADER pro Member
```

`Xcdr2Writer.beginAppendable()` / `beginMutable()` /
`writeEmHeader(id, lc)` sind Helper-Methoden.

## §7 Key-Extraction

```ts
keyHash(s: Sensor): Uint8Array {
    const w = new Xcdr2Writer('be');
    w.writeInt32(s.id); // @key
    return md5(w.toBytes());
}
```

`md5` ist eine pure-TS RFC-1321-Implementation (kein Web-Crypto-API
weil sync und ohne Promise gebraucht).

## §8 Helper-Library `@zerodds/cdr`

`crates/ts-node/src/cdr/` (zugleich publish-Source fuer NPM):

| Datei | Inhalt |
|-------|--------|
| `index.ts` | Re-Exports |
| `types.ts` | `DdsTopicType<T>`, `ExtensibilityKind`, `EndianMode` |
| `writer.ts` | `Xcdr2Writer` |
| `reader.ts` | `Xcdr2Reader` |
| `md5.ts` | RFC 1321 |
| `errors.ts` | `XcdrError extends Error` |

Pure TypeScript ≥ 5.0. Browser- und Node-Target gleichermassen
(`Uint8Array` + `DataView` sind universal). Keine `Buffer`-Dependency.

ts-wasm-Variante (`crates/ts-wasm/src/cdr/`) ist binary-identisch ueber
re-export; WASM-Module brauchen den Codec nicht selbst — TS-Layer
serialisiert ausserhalb des WASM-Bindings.

## §9 Conformance

L1-L4 gegen [`zerodds-xcdr2-bindings-conformance-1.0`](zerodds-xcdr2-bindings-conformance-1.0.md):

- L1 (Wire): `crates/ts-node/test/xcdr2-wire-vectors.test.ts`
  prueft V-1..V-12 (`vitest` oder `mocha`).
- L2 (Codegen): `crates/idl-ts/tests/snapshots/` mit generierten
  `*.ts`-Files.
- L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs`
  ruft `node --import tsx ts-runner.ts`.
- L4 (Cross-Vendor): TS Encoder ueber FFI in zerodds-c-api → Cyclone.

## §10 Examples

`crates/ts-node/examples/topic-typed-smoke.ts` ist Referenz-Smoke
(generierter `PointTypeSupport` + Pub/Sub-Loop).

## §11 Errata + Open-Questions

- **§11.1 `Number` Praezision**: int32/uint32 passen in 53-bit Mantisse.
  int64/uint64 brauchen `bigint`. Codegen emittiert striktes Type.
- **§11.2 UTF-8 Encoding**: `TextEncoder('utf-8')` ist universal
  (Node ≥ 12, alle Browser). Helper kapselt das.
- **§11.3 ESM vs CJS**: NPM-Paket emittiert dual (ESM + CJS) via
  `"exports"`-Feld; Codegen-Output ist ESM-only (TypeScript-Sources).
- **§11.4 Tree-Shaking**: Per-Type `*TypeSupport`-Const wird beim
  Bundlen nicht eliminiert wenn der Type benutzt wird; Side-effect-frei
  via `// @__PURE__`-Annotations.
- **§11.5 Browser-CSP**: MD5 als pure-TS implementiert (kein
  WebCrypto-Async-API), damit `keyHash` synchron bleibt.
