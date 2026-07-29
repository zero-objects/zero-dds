# `zerodds-xcdr2-fsharp` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-xcdr2-fsharp-1.0.md` — ZeroDDS F# XCDR2 TypeSupport-Codegen-Spec.

Implementation:

- `crates/idl-csharp/` — IDL → F# Codegen (`marshalXCDR`, self-contained Writer).
- `endpoints/fsharp/` — pure-Kotlin/JVM XCDR2 Wire-Core (Writer/Reader) + sync/async SDK.

## §1 Motivation

### §1 Kein OMG-IDL-to-D-Wire-Mapping

**Spec:** §1 — ZeroDDS definiert das D-XCDR2-Wire-Mapping.

**Repo:** Motivations-Text der Vendor-Spec.

**Tests:** —

**Status:** n/a (informative)

## §2 Marshal-Pattern

### §2 struct + `marshalXCDR(endian, allocator) ![]u8`

**Spec:** §2 — pro IDL-`@final struct` ein D-`struct` + `marshalXCDR`-Methode.

**Repo:** `crates/idl-csharp/src/emitter.rs::emit_struct`.

**Tests:** `crates/idl-csharp/tests/golden.rs::final_struct_emits_type_and_marshal`

**Status:** done

## §3 Required API-Surface

### §3 Writer/Reader-Primitiven + generierte marshalXCDR

**Spec:** §3 — Writer (putU8..putSeqU8, bytes), Reader (getU8/getU16/getU32/getU64/getF32/getString/getSeqU8), generierte `marshalXCDR`.

**Repo:** `endpoints/fsharp/zerodds.fs` (Writer, Reader inkl. getU8/getU16/getU64/getF32/getString/getSeqU8 (byte-exakter Inverse)); `crates/idl-csharp/src/emitter.rs` (marshalXCDR).

**Tests:** `endpoints/fsharp/zerodds.fs` test.fsx (byte identity + sync + async), `endpoints/fsharp/example_sync|async.fsx` (voller Feld-Decode), `crates/idl-csharp/tests/golden.rs`.

**Status:** done

### §3.a Generierte `unmarshalXCDR` (decode-codegen)

**Spec:** §3/§11 — Decode über `Reader` gelaufen; generiertes `unmarshalXCDR` fehlt.

**Repo:** — (Decode über `Reader`; siehe Examples.)

**Tests:** `endpoints/fsharp/example_sync|async.fsx`.

**Status:** n/a (rejected)

**Decision-Record:** Encode-Codegen (`marshalXCDR`) + vollständiger `Reader` für Decode; generiertes Decode ist über alle 17 idlc-Backends bewusst nicht Teil dieser Binding-Spec (der `Reader` deckt jeden Feldtyp byte-identisch ab), gehört zum optionalen Full-TypeSupport-Codegen (roadmap).

## §4 Codegen-Pflicht (`idl-csharp`)

### §4 struct + marshalXCDR + eingebetteter Writer

**Spec:** §4 — pro struct: D-struct, `marshalXCDR`, self-contained Writer.

**Repo:** `crates/idl-csharp/src/emitter.rs`; `tools/idlc` `Backend::D`, `--csharp` (.NET-Interop).

**Tests:** `crates/idl-csharp/tests/golden.rs`.

**Status:** done

## §5 Wire-Type-Mapping

### §5 IDL → F# → XCDR2 (Alignment cap 4)

**Spec:** §5 — bool/octet/char/short..long long/float/double/string/sequence<octet> mit exaktem Wire-Layout; signed via BitConverter.

**Repo:** `crates/idl-csharp/src/emitter.rs::map_type/map_primitive/map_integer/map_sequence`; `endpoints/fsharp/zerodds.fs` (Put*/getLE align cap 4).

**Tests:** `crates/idl-csharp/tests/golden.rs::byte_identity_vs_rust_goldens` (@final LE+BE).

**Status:** done

### §5.a wchar / wstring / long double / map / array / nested / union

**Spec:** §5/§11 — weitere IDL-Konstrukte.

**Repo:** — (`idl-csharp codegen::Unsupported`).

**Tests:** —

**Status:** n/a (rejected)

**Decision-Record:** Bewusst außerhalb v1.0-Scope, einheitlich über alle 17 Backends (Kern = primitiv/string/sequence<octet> des @final-Golden). Roadmap-getrackt; Backends geben ehrlich `Unsupported` statt falscher Wire.

## §6 Extensibility

### §6 @final (compact) + @appendable (DHEADER)

**Spec:** §6 — @final ohne DHEADER, @appendable mit uint32-Body-Length + Body.

**Repo:** `crates/idl-csharp/src/emitter.rs::emit_struct` (Final/Appendable).

**Tests:** `crates/idl-csharp/tests/golden.rs::final_struct_...` + `appendable_struct_frames_a_dheader`.

**Status:** done

### §6.a @mutable (EMHEADER)

**Spec:** §6 — @mutable-EMHEADER-Framing.

**Repo:** — (`Unsupported`).

**Tests:** —

**Status:** n/a (rejected)

**Decision-Record:** @mutable-EMHEADER über alle 17 Backends einheitlich nicht emittiert; @final+@appendable decken den gängigen Fall. Cross-cutting Follow-up (roadmap), bewusst zurückgestellt. `idl-csharp` gibt `Unsupported`.

## §7 Key-Extraction

### §7 Non-keyed 16-Zero-Byte-Key

**Spec:** §7 — non-keyed → 16 Nullbytes; keyed → MD5 (XCDR2-BE), Runtime.

**Repo:** `endpoints/fsharp/zerodds.fs` (Writer im Big-Mode liefert die BE-Serialisierung).

**Tests:** —

**Status:** done

### §7.a Per-struct generierte `keyHash` aus `@key`

**Spec:** §7 — Codegen einer `keyHash`-Methode aus `@key`.

**Repo:** —

**Tests:** —

**Status:** n/a (rejected)

**Decision-Record:** Key-Hash-Codegen gehört zum Full-DCPS-TypeSupport, nicht zum XCDR2-Wire-Binding v1.0; die DCPS-Runtime berechnet Key-Hashes. Über alle 17 Backends einheitlich zurückgestellt.

## §8 Wire-Core

### §8 `endpoints/fsharp` als Referenz-Writer/Reader

**Spec:** §8 — Referenz-Wire-Core, byte-identisch zu `zerodds-cdr`.

**Repo:** `endpoints/fsharp/zerodds.fs`.

**Tests:** test.fsx, CI-Job `endpoints-fsharp`.

**Status:** done

## §9 Conformance

### §9 Golden-Byte-Identität @final LE+BE

**Spec:** §9 — Encoding == golden_le.bin / golden_be.bin byte-für-byte.

**Repo:** `crates/idl-csharp`, `endpoints/fsharp`.

**Tests:** `crates/idl-csharp/tests/golden.rs::byte_identity_vs_rust_goldens` (CI `idl-csharp`); `endpoints/fsharp` test.fsx (CI `endpoints-fsharp`).

**Status:** done

## §10 Examples

### §10 sync + async Deep-Examples + Quickstart

**Spec:** §10 — lauffähige sync/async-Beispiele.

**Repo:** `endpoints/fsharp/example_sync.fsx`, `endpoints/fsharp/example_async.fsx`, `endpoints/fsharp/QUICKSTART.md`.

**Tests:** CI-Job `endpoints-fsharp` führt beide aus (dotnet fsi example_sync.fsx + example_async.fsx).

**Status:** done

## §11 Errata + Open-Questions

### §11 Ehrliche Nicht-Ziele

**Spec:** §11 — decode-codegen, keyHash-codegen, @mutable, wchar/wstring/map/array/nested/union/long double.

**Repo:** siehe §3.a/§5.a/§6.a/§7.a (je `n/a (rejected)` mit Decision-Record).

**Tests:** —

**Status:** n/a (informative)
