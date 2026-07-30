# `zerodds-xcdr2-node` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-node-1.0.md` — ZeroDDS Node XCDR2 TypeSupport-Codegen-Spec.

Implementation:

- `crates/idl-ts/` — IDL → Node Codegen (`marshalXCDR`, self-contained Writer).
- `endpoints/node/` — pure-Kotlin/JVM XCDR2 Wire-Core (Writer/Reader) + sync/async SDK.

## §1 Motivation

### §1 Kein OMG-IDL-to-D-Wire-Mapping

**Spec:** §1 — ZeroDDS definiert das D-XCDR2-Wire-Mapping.

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal-Pattern

### §2 struct + `marshalXCDR(endian, allocator) ![]u8`

**Spec:** §2 — pro IDL-`@final struct` ein D-`struct` + `marshalXCDR`-Methode.

**Repo:** `crates/idl-ts/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-ts/tests/golden.rs::final_struct_emits_type_and_marshal`

**Status:** done

## §3 Required API-Surface

### §3 Writer/Reader-Primitiven + generierte marshalXCDR

**Spec:** §3 — Writer (putU8..putSeqU8, bytes), Reader (getU8/getU16/getU32/getU64/getF32/getString/getSeqU8), generierte `marshalXCDR`.

**Repo:** `endpoints/node/zerodds.js` (Writer, Reader inkl. getU8/getU16/getU64/getF32/getString/getSeqU8 (byte-exakter Inverse)); `crates/idl-ts/src/emitter.rs` (marshalXCDR).

**Tests:** `endpoints/node/zerodds.js` test.js (byte identity + sync + async), `endpoints/node/example_sync|async.js` (voller Feld-Decode), `crates/idl-ts/tests/golden.rs`.

**Status:** done

### §3.a Generierte `unmarshalXCDR` (decode-codegen)

**Spec:** §3/§11 — Decode über `Reader` gelaufen; generiertes `unmarshalXCDR` fehlt.

**Repo:** — (Decode über `Reader`; siehe Examples.)

**Tests:** `endpoints/node/example_sync|async.js`.

**Status:** n/a (rejected)

**Decision-Record:** Encode-Codegen (`marshalXCDR`) + vollständiger `Reader` für Decode; generiertes Decode ist über alle 17 idlc-Backends bewusst nicht Teil dieser Binding-Spec (der `Reader` deckt jeden Feldtyp byte-identisch ab), gehört zum optionalen Full-TypeSupport-Codegen (roadmap).

## §4 Codegen-Pflicht (`idl-ts`)

### §4 struct + marshalXCDR + eingebetteter Writer

**Spec:** §4 — pro struct: D-struct, `marshalXCDR`, self-contained Writer.

**Repo:** `crates/idl-ts/src/emitter.rs`; `tools/idlc` `Backend::D`, `--ts` (TypeScript).

**Tests:** `crates/idl-ts/tests/golden.rs`.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL → Node → XCDR2 (Alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> mit exaktem Wire-Layout; signed via Buffer.writeFloatLE.

**Repo:** `crates/idl-ts/src/emitter.rs::map_type/map_primitive/map_integer/map_sequence`; `endpoints/node/zerodds.js` (Put*/getLE align cap 4).

**Tests:** `crates/idl-ts/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a wchar / wstring / long double / map / array / nested / union

**Spec:** §5/§11 — weitere IDL-Konstrukte.

**Repo:** `crates/idl-ts/src/` (union-, `map<>`-, Array-, Nested-Struct-, `wchar`/`wstring`/`long double`-Emit-Arme).

**Tests:** —

**Status:** done

**Decision-Record:** Das `idl-ts`-Codegen emittiert diese Konstrukte (Unions, `map<>`, Arrays, Nested-Struct-Member, `wchar`, `wstring`, `long double`). Nur `@mutable`-Unions bleiben zurückgestellt (siehe §6.a).

## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final ohne DHEADER, @appendable mit uint32-Body-Length + Body.

**Repo:** `crates/idl-ts/src/emitter.rs::emit_struct` (Final/Appendable).

**Tests:** `crates/idl-ts/tests/golden.rs::final_struct_...` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable-EMHEADER-Framing.

**Repo:** `crates/idl-ts/src/` (`@mutable` PL_CDR2-EMHEADER- / PL_CDR1-Pfade).

**Tests:** —

**Status:** done (Structs); `@mutable`-Unions zurückgestellt

**Decision-Record:** `@mutable`-Structs werden emittiert — PL_CDR2-EMHEADER-Framing unter XCDR2, PL_CDR1 unter XCDR1. Nur `@mutable`-Unions werden noch nicht emittiert → `Unsupported`.

## §7 Key-Extraction

### §7 Non-keyed 16-Zero-Byte-Key

**Spec:** §7 — non-keyed → 16 Nullbytes; keyed → MD5 (XCDR2-BE), Runtime.

**Repo:** `endpoints/node/zerodds.js` (Writer im Big-Mode liefert die BE-Serialisierung).

**Tests:** —

**Status:** done

### §7.a Per-struct generierte `keyHash` aus `@key`

**Spec:** §7 — Codegen einer `keyHash`-Methode aus `@key`.

**Repo:** —

**Tests:** —

**Status:** n/a (rejected)

**Decision-Record:** Key-Hash-Codegen gehört zum Full-DCPS-TypeSupport, nicht zum XCDR2-Wire-Binding v1.0; die DCPS-Runtime berechnet Key-Hashes. Über alle 17 Backends einheitlich zurückgestellt.

## §8 Wire-Core

### §8 `endpoints/node` als Referenz-Writer/Reader

**Spec:** §8 — Referenz-Wire-Core, byte-identisch zu `zerodds-cdr`.

**Repo:** `endpoints/node/zerodds.js`.

**Tests:** test.js, CI-Job `endpoints-node`.

**Status:** done

## §9 Conformance

### §9 Golden-Byte-Identität @final LE+BE

**Spec:** §9 — Encoding == golden_le.bin / golden_be.bin byte-für-byte.

**Repo:** `crates/idl-ts`, `endpoints/node`.

**Tests:** `crates/idl-ts/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-ts`); `endpoints/node` test.js (CI `endpoints-node`).

**Status:** done

## §10 Examples

### §10 sync + async Deep-Examples + Quickstart

**Spec:** §10 — lauffähige sync/async-Beispiele.

**Repo:** `endpoints/node/example_sync.js`, `endpoints/node/example_async.js`, `endpoints/node/QUICKSTART.md`.

**Tests:** CI-Job `endpoints-node` führt beide aus (node example_sync.js + example_async.js).

**Status:** done

## §11 Errata + Open-Questions

### §11 Ehrliche Nicht-Ziele

**Spec:** §11 — decode-codegen, keyHash-codegen, `@mutable`-Unions.

**Repo:** siehe §3.a und §7.a (je `n/a (rejected)` mit Decision-Record); §5.a und §6.a sind jetzt `done`.

**Tests:** —

**Status:** n/a (informative)
