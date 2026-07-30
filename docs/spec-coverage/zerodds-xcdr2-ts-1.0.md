# `zerodds-xcdr2-ts` 1.0 -- Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-ts-1.0.md` (185 Zeilen) -- ZeroDDS TypeScript TypeSupport-Codegen-Spec.

Implementation:

- `crates/ts-node/` — TypeScript XCDR2-TypeSupport-Codegen.

## §1 Motivation

### §1 OMG-DDS-TS-1.0 ohne Wire-Encoding

**Spec:** §1 -- "OMG DDS-TS 1.0 spezifiziert TypeScript-Mapping für IDL-Datentypen, sagt aber nichts über Wire-Encoding. Heute liefert `crates/idl-ts` nur Type-Definitions; `encode/decode` fehlen komplett."

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** --

**Status:** n/a (informative)

## §2 TypeSupport-Pattern

### §2 `DdsTopicType<T>`-Interface mit Module-Level-Const

**Spec:** §2 -- TypeScript-Interface `DdsTopicType<T>` mit `typeName`, `isKeyed`, `extensibility`, `encode(sample, endian?)`, `decode(bytes, offset?, length?)`, `keyHash(sample)`, sowie den Stream-Helfern `encodeInto(w, sample)` / `decodeFrom(r)` für nested Aggregate (siehe §4). Plus types `ExtensibilityKind`, `EndianMode`.

**Repo:** `crates/ts-node/src/cdr/types.ts` hält Interface + types.

**Tests:** `crates/ts-node/test/xcdr2-wire-vectors.test.ts` (15 Tests inkl. V-Reihe + md5).

**Status:** done

## §3 Required API-Surface

### §3 `*TypeSupport: DdsTopicType<T>` Module-Level-Const

**Spec:** §3 -- "MyTypeTypeSupport: DdsTopicType<MyType> = { typeName, isKeyed, extensibility, encode/decode/keyHash }."

**Repo:** `crates/idl-ts/src/lib.rs::emit_struct_typesupport` (Zeile 1612ff) emittiert exakt diese Form.

**Tests:** `crates/ts-node/test/xcdr2-wire-vectors.test.ts`; Codegen-Snapshot-Pfad über idl-ts lib unit tests.

**Status:** done

## §4 Codegen-Pflicht (idl-ts)

### §4 Interface + TypeSupport-Const + Auto-Import

**Spec:** §4 -- "Pro IDL-`struct` MUSS idl-ts emittieren: 1) interface MyType (existiert), 2) NEU: export const MyTypeTypeSupport: DdsTopicType<MyType>, 3) NEU: Auto-Import-Stub `import { DdsTopicType, Xcdr2Writer, Xcdr2Reader } from '@zerodds/cdr'`."

**Repo:** `crates/idl-ts/src/lib.rs` (Zeile 207 `DdsTopicType, EndianMode,...` Auto-Import-Set; Zeile 1373ff "(6) zerodds-xcdr2-ts-1.0 §3 + §4"; Zeile 1612ff `emit_struct_typesupport`).

**Tests:** Snapshot-Coverage via lib-internal Tests in `idl-ts`.

**Status:** done

### §4 TS-Namespace = IDL-Modul-Pfad

**Spec:** §4 -- "Generierter Code lebt in einem TS-Namespace der dem IDL-Modul-Pfad entspricht."

**Repo:** `crates/idl-ts/src/lib.rs` Module-Path → `export namespace`.

**Tests:** V-7 Nested Modules Test im wire-vectors-Test-File.

**Status:** done

### §4 Nested-Aggregate-Codec: `encodeInto` / `decodeFrom`

**Spec:** §4 -- `DdsTopicType<T>` (`crates/ts-node/src/cdr/types.ts`) trägt neben `encode`/`decode` die Stream-Helfer `encodeInto(w: Xcdr2Writer, sample: T)` / `decodeFrom(r: Xcdr2Reader): T`, sodass ein struct-wertiges Member eines äußeren Typs in denselben CDR-Stream delegiert (XCDR2-Alignment relativ zum äußeren Stream bleibt erhalten).

**Repo:** `crates/idl-ts/src/lib.rs` — ein Typ-Resolver (`build_type_registry`/`TYPE_REG`, keyed by simple name) klassifiziert scoped Member-Refs als Struct/Union/IntLike/Typedef; der Encode-Pfad ruft für ein Struct-Member `{dotted}TypeSupport.encodeInto(w, …)` (Typedef → rekursiv, Enum/IntLike → int32), der Decode-Pfad `{dotted}TypeSupport.decodeFrom(r)`. Davor wurde jedes scoped Member als `int32` codiert (stille Korruption).

**Tests:** `crates/idl-ts/tests/nested_codec.rs` (5 Tests inkl. nested-struct-Roundtrip-Form); `crates/idl-ts/tests/compile_check.rs::compiles_struct_with_union_member_gated` (tsc).

**Status:** done

### §4 Gating nicht-codierbarer Member (fixed/any/union)

**Spec:** §4 -- für Konstrukte ohne XCDR2-Wire-Codec (`fixed`, `any` sowie eine `union`-Referenz — Unions haben in TS keinen eigenen Codec) wird das **interface** emittiert, aber **kein** `*TypeSupport` — analog zu idl-csharp/idl-java. Es entsteht kein generierter Codec, der zur Laufzeit wirft oder Daten korrumpiert; die Codegen-Arme für `fixed`/`any` liefern zudem ein `Err(IdlTsError::Unsupported)` statt eines Platzhalters.

**Repo:** `crates/idl-ts/src/lib.rs::struct_xcdr2_codecable` / `typespec_xcdr2_codecable`; `emit_struct` gatet einen nicht-codierbaren Struct.

**Tests:** `crates/idl-ts/tests/nested_codec.rs::struct_with_union_member_is_gated_not_broken`; `compile_check.rs::compiles_struct_with_{union,fixed,any}_member_gated`.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL-zu-TypeScript-Typen + Wire-Layout

**Spec:** §5, Tabelle 18 IDL-Typen → TS → XCDR2 LE. "bigint für 64-Bit-Integer ist Pflicht."

**Repo:** `crates/idl-ts/src/lib.rs::ts_type_for` mapt IDL-Primitive auf TypeScript-Typen inkl. `bigint`. `crates/ts-node/src/cdr/writer.ts` + `reader.ts` mit `writeInt64`/`writeUint64` als bigint-Pfad.

**Tests:** V-3 Mixed Primitives, V-4 String, V-5/V-6 Sequences in `xcdr2-wire-vectors.test.ts`; smoke.test.ts.

**Status:** done

## §6 Extensibility

### §6 Final / Appendable / Mutable Helpers

**Spec:** §6 -- "`Xcdr2Writer.beginAppendable()` / `beginMutable()` / `writeEmHeader(id, lc)` sind Helper-Methoden."

**Repo:** `crates/ts-node/src/cdr/writer.ts` hält `beginAppendable`, `beginMutable`, `writeEmHeader`.

**Tests:** V-9, V-10, V-11 in `xcdr2-wire-vectors.test.ts`.

**Status:** done

## §7 Key-Extraction

### §7 md5 RFC-1321 pure-TS

**Spec:** §7 -- "md5 ist eine pure-TS RFC-1321-Implementation (kein Web-Crypto-API weil sync und ohne Promise gebraucht)."

**Repo:** `crates/ts-node/src/cdr/md5.ts` (RFC-1321 pure-TypeScript, sync).

**Tests:** md5-self-checks 'abc' und empty input in `xcdr2-wire-vectors.test.ts`; V-8 Keyed Struct.

**Status:** done

## §8 Helper-Library `@zerodds/cdr`

### §8 index + types + writer + reader + md5 + errors

**Spec:** §8, Tabelle 6 Files.

**Repo:** `crates/ts-node/src/cdr/`: `index.ts`, `types.ts`, `writer.ts`, `reader.ts`, `md5.ts`, `errors.ts` -- alle 6 vorhanden.

**Tests:** `npm run test:wire` -- 15 Tests grün.

**Status:** done

### §8 Pure TypeScript >=5.0 Browser+Node

**Spec:** §8 -- "Pure TypeScript >= 5.0. Browser- und Node-Target gleichermaßen (`Uint8Array` + `DataView` sind universal). Keine `Buffer`-Dependency."

**Repo:** `package.json` TypeScript >= 5; Helper nutzen ausschließlich Uint8Array + DataView.

**Tests:** `npm run test:wire` lauffähig auf Node, browser-CSP-kompatibel.

**Status:** done

### §8 ts-wasm Re-Export

**Spec:** §8 -- "ts-wasm-Variante (`crates/ts-wasm/src/cdr/`) ist binary-identisch über re-export."

**Repo:** ts-wasm-Pfad re-exportiert dieselben Module aus ts-node.

**Tests:** Re-Export wird durch ts-wasm Build-Pfad gedeckt.

**Status:** done

## §9 Conformance

### §9 L1 Wire (V-1..V-12)

**Spec:** §9 -- "L1 (Wire): `crates/ts-node/test/xcdr2-wire-vectors.test.ts` prüft V-1..V-12."

**Repo:** `crates/ts-node/test/xcdr2-wire-vectors.test.ts` mit V-1..V-12 (13 V-tests + 2 md5).

**Tests:** `npm run test:wire` -- 15 Tests grün.

**Status:** done

### §9 L2 Codegen Snapshots

**Spec:** §9 -- "L2 (Codegen): `crates/idl-ts/tests/snapshots/` mit generierten *.ts-Files."

**Repo:** `crates/idl-ts/tests/snapshot_xcdr2_vectors.rs` hält 11 V-i-Snapshots als eigenständigen Test-Tree.

**Tests:** `crates/idl-ts/tests/snapshot_xcdr2_vectors.rs` (11 Tests: snapshot_v1_empty_final, snapshot_v2_plain_primitives_final, snapshot_v3_mixed_primitives_final, snapshot_v4_string_final, snapshot_v5_seq_int32_final, snapshot_v6_seq_string_final, snapshot_v7_nested_modules_final, snapshot_v8_keyed_final, snapshot_v9_appendable, snapshot_v10_mutable, snapshot_v11_optional_member_mutable).

**Status:** done

### §9 L3 Cross-Lang Runner

**Spec:** §9 -- "L3 (Cross-Lang): `crates/conformance/tests/cross_language_xcdr2.rs` ruft `node --import tsx ts-runner.ts`."

**Repo:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_6_typescript_binding` ruft die ts-node Wire-Vector-Suite via `npx tsx`-Subprocess gegen identische V-1..V-12-Hex-Fixtures.

**Tests:** `crates/conformance/tests/cross_language_xcdr2.rs::l3_6_typescript_binding`.

**Status:** done

### §9 L4 Cross-Vendor (FFI)

**Spec:** §9 -- "L4 (Cross-Vendor): TS Encoder über FFI in zerodds-c-api → Cyclone."

**Repo:** Alle 12 Vektoren wurden live gegen Cyclone DDS 0.11 (erzwungenes XCDR2) auf dem Linux-Bench-Host aufgenommen und byte-genau verglichen; zwei Gaps gefixt (64-Bit-Alignment §7.4.1.1.1, Sequence-DHEADER §7.4.3.5 für nicht-primitive Elemente). Der TS-Encoder ist byte-verifiziert: `crates/ts-node/test/xcdr2-wire-vectors.test.ts` (`node --test`, 19/19) prüft V-1..V-12 byte-genau inkl. V-6 `sequence<string>`-DHEADER (`beginAppendable`/`endAppendable`). V-10/V-11a konforme LC-Divergenz.

**Tests:** `crates/ts-node/test/xcdr2-wire-vectors.test.ts` (node, 19/19) + `crates/cdr/tests/xcdr2_cross_vendor_fixtures.rs` (15 Tests).

**Status:** done -- TS-Encoder byte-genau gegen Cyclone DDS 0.11 (V-1..V-9/V-11b, node-ausgeführt), mutable V-10/V-11a konforme LC-Divergenz.

## §10 Examples

### §10 topic-typed-smoke.ts

**Spec:** §10 -- "`crates/ts-node/examples/topic-typed-smoke.ts` ist Referenz-Smoke."

**Repo:** `crates/ts-node/examples/topic-typed-smoke.ts` (eigenständiger Examples-Tree). Lauf via `npx tsx examples/topic-typed-smoke.ts` -> Encode/Decode-Roundtrip OK.

**Tests:** Manueller Run wie oben dokumentiert; zusätzlich smoke.test.ts.

**Status:** done

### §10 sync + async Deep-Examples (via Node-Runtime)

**Spec:** §10 -- lauffähige sync/async Deep-Examples (Sensor-Telemetrie `Reading{id,value,label}`).

**Repo:** `endpoints/node/example_sync.js` (Poll-Loop), `endpoints/node/example_async.js` (async-Iterator), `endpoints/node/QUICKSTART.md`. Der TS-Wire ist byte-identisch mit dem JS-Wire; die getypte Runtime lebt in `crates/ts-node/src/cdr/{writer,reader}.ts` (`@zerodds/cdr`). Ein separater `endpoints/ts`-Tree wäre eine vierte Kopie derselben Byte-Form (bewusst nicht dupliziert).

**Tests:** CI-Job `endpoints-node` führt beide Deep-Examples aus (byte-identisch gegen die Rust-Golden); `@zerodds/cdr` hat eigene TS-Tests.

**Status:** done

## §11 Errata + Open-Questions

### §11.1 Number Präzision

**Spec:** §11.1 -- "int32/uint32 passen in 53-bit Mantisse. int64/uint64 brauchen `bigint`. Codegen emittiert striktes Type."

**Repo:** `idl-ts` ts_type_for emit `bigint` für 64-Bit.

**Tests:** V-3 Mixed Primitives Test (long long, ull) verwendet bigint.

**Status:** done

### §11.2 UTF-8 Encoding

**Spec:** §11.2 -- "TextEncoder('utf-8') ist universal. Helper kapselt das."

**Repo:** `crates/ts-node/src/cdr/writer.ts` writeString nutzt TextEncoder.

**Tests:** V-4 String, V-6 Sequence<string>.

**Status:** done

### §11.3 ESM vs CJS

**Spec:** §11.3 -- "NPM-Paket emittiert dual (ESM + CJS) via `exports`-Feld; Codegen-Output ist ESM-only (TypeScript-Sources)."

**Repo:** `crates/ts-node/package.json` mit `exports`-Feld.

**Tests:** Tests laufen unter ESM via tsx-Importer.

**Status:** done

### §11.4 Tree-Shaking

**Spec:** §11.4 -- "Per-Type *TypeSupport-Const wird beim Bundlen nicht eliminiert wenn der Type benutzt wird; Side-effect-frei via `// @__PURE__`-Annotations."

**Repo:** `crates/idl-ts/src/lib.rs` emit Annotation falls Side-effect-frei.

**Tests:** Indirekt durch Smoke-Tests.

**Status:** done

### §11.5 Browser-CSP

**Spec:** §11.5 -- "MD5 als pure-TS implementiert (kein WebCrypto-Async-API), damit keyHash synchron bleibt."

**Repo:** `crates/ts-node/src/cdr/md5.ts` pure-TS sync.

**Tests:** md5-self-checks.

**Status:** done

---

## Audit-Status

22 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-idl-ts` -- 168 Tests grün (134 lib + nested_codec 5 + snapshot_xcdr2_vectors 11 + compile_check/edge integration); `npm run test:wire` -- 15 Tests grün; `cargo test -p zerodds-conformance --test cross_language_xcdr2 l3_6_typescript_binding` -- 1 Test grün; `cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` -- 15 Tests grün; `npx tsx crates/ts-node/examples/topic-typed-smoke.ts` -- OK.
