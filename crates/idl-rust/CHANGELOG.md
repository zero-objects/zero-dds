# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed

- **`@mutable` encoder lacked DHEADER wrapping**
  (`src/struct_emit.rs::emit_encode_body`). The encode side emitted
  only the EMHEADER list, while the decode side already used
  `decode_appendable` (DHEADER-aware). Generated code could encode
  but not decode its own output. Both sides now wrap the body
  symmetrically. Spec anchor: OMG XTypes 1.3 §7.4.3.4.4 + master
  conformance §6 V-10.

### Changed

- **`TYPE_NAME` now uses fully-qualified IDL scoped name**
  with `::` module separator (`Module::Sub::Struct`). Threading via
  `src/emitter.rs::emit_module` (raw IDL module names) into
  `src/struct_emit.rs::emit_dds_type_impl`. Spec: master conformance
  §3 / §5 / V-7. Snapshot: `snapshot_codegen__snapshot_module_nested.snap`.
- **`EXTENSIBILITY` const emitted per generated `impl DdsType`**.
  One of `Extensibility::Final` / `::Appendable` / `::Mutable` per
  XTypes 1.3 §7.4.5. Spec: zerodds-xcdr2-rust §2.3 + §6.

### Notes

- See `docs/spec-coverage/zerodds-xcdr2-rust-1.0.md` for the per-
  section coverage map and §11 errata (binding-impl vs spec naming
  aliases for `IS_KEYED` / `HAS_KEY`, `key_hash` /
  `compute_key_hash`, `ExtensibilityKind` / `Extensibility`).

## [1.0.0-rc.1]

Initiale Release-Materialisierung der `zerodds-idl-rust`-Crate. Schließt die Architektur-Lücke „pure-Rust DDS-Stack ohne Rust-Codegen". Ergänzt die anderen Sprach-Codegens (`idl-cpp` / `idl-csharp` / `idl-java` / `idl-ts`) um den Rust-Pfad.

### Spec-Referenzen

- **OMG IDL4** (formal/2018-07-01) §7.4 — Type-System, Annotations, Modules.
- **OMG XTypes 1.3** (formal/2024-04-04) §7.4 — XCDR2 Wire-Format, Extensibility (final / appendable / mutable), §7.4.5.1 Enum-Encoding (i32), §7.6.8 KeyHash + KeyHolder-Member-ID-Sortierung.
- **OMG DDS 1.4** (formal/2015-04-10) §2.2.3.4 — DataType-Topic-Registration via `DdsType::TYPE_NAME`.

### Public-API

- `generate_rust_module(spec, opts)` — AST → Rust-String (zentrale Entry-Funktion).
- `RustGenOptions { header_comment }` — Codegen-Optionen.
- `error::RustGenError` — Fehler-Familie (`Unsupported`, `InvalidAnnotation`, `UnresolvedType`).
- `error::Result<T>` — Result-Alias.

Modul-Struktur:
- `emitter` — Top-Level AST-Walker (Module + Definitionen).
- `struct_emit` — Struct + DdsType-Impl (encode/decode/encode_key_holder_be).
- `enum_emit` — Enum + CdrEncode/CdrDecode-Impl + `from_wire`.
- `union_emit` — Union → Rust-Enum mit Discriminator.
- `typedef_emit` — typedef → `pub type X = Y;`.
- `type_map` — IDL-Type → Rust-Type Mapping inkl. wire-size-bound für KeyHolder-Berechnung.
- `annotations` — `@key`/`@id`/`@final`/`@appendable`/`@mutable`/`@extensibility`/`@must_understand`/`@nested`/`@optional`.

### Implementierung

Codegen-Strategie ist **string-basiert** analog zu den anderen idl-* Crates (kein quote/syn) — ein Rust-Codegen für Rust-Code muss keine Macro-Hygiene haben, die Output-Strings werden vom rustc geparst und Type-Errors gegen den emittierten Code gefangen.

Encode/Decode-Bodies nutzen einheitlich den `zerodds_cdr::CdrEncode`/`CdrDecode`-Trait-Pfad statt method-calls — alle Primitives und Composite-Types haben `impl CdrEncode` in `zerodds_cdr::encode` / `zerodds_cdr::composite`. Damit emittiert der Codegen denselben Pattern für `i32` (primitiv), `String` (composite), `Vec<T>` (composite) und nested Structs (Trait-Auto-Resolve).

Extensibility-Modi werden auf die `zerodds_cdr::struct_enc`-Helper abgebildet:
- `@final` (default): direktes encode in deklarations-Reihenfolge.
- `@appendable`: `zerodds_cdr::struct_enc::encode_appendable(writer, |w| { ... })` mit DHEADER-Wrap.
- `@mutable`: `zerodds_cdr::struct_enc::MutableStructEncoder` mit per-Member-ID + LengthCode.

@key-Felder werden in `encode_key_holder_be` member-id-sortiert geschrieben (Spec §7.6.8.3.1.b). `KEY_HOLDER_MAX_SIZE` wird zur Compile-Zeit aus den Wire-Sizes der @key-Member berechnet — fixed-size-Members liefern einen `Some(n)`-Bound (zero-pad-Pfad), variable-size-Members (String, Vec) liefern `None` (MD5-Pfad in `compute_key_hash`).

`From<zerodds_cdr::EncodeError>` und `From<zerodds_cdr::DecodeError>` sind in `zerodds_dcps::dds_type` implementiert, damit der `?`-Operator im generierten Code transparent zwischen den beiden Error-Hierarchien konvertiert.

### Architektur

- **Layer:** 3 (Schema).
- **Dependencies (in):** `zerodds-idl` (AST + Parser).
- **Dependents (out):** End-User-Build-Skripte (typisch in einem `build.rs` oder als CLI-Tool).
- **Feature-Flags:** keine — std-only Build-Zeit-Tool.

### Tests

- **Snapshot-Tests** in `tests/snapshot_codegen.rs` — 13 Tests, jeder Snapshot ist committed unter `tests/snapshots/`. Abdeckt: simple struct, full primitive set, enum, typedef, module-nested, appendable, mutable mit @id, struct mit String+sequence, struct mit Multi-Dim-Array, union, single-key, multi-key mit @id-Sortierung, string-key (unbounded).
- **Compile-Check-Tests** in `tests/compile_check.rs` (`#[ignore]`-gated) — 8 Tests, jeder schreibt den emittierten Code in eine temp-Crate mit Pfad-Deps auf `zerodds-cdr`+`zerodds-dcps` und ruft `cargo check` auf. Belegt End-to-End: der Codegen-Output ist real-kompilierbar gegen den ZeroDDS-Stack.

```bash
cargo test -p zerodds-idl-rust --tests
cargo test -p zerodds-idl-rust --test compile_check -- --include-ignored
```

### Stabilität

Alle `pub`-Items sind RC1-stabil; Breaking-Changes erfordern Major-Bump. Der **emittierte Rust-Code** hat eine eigene Stabilitäts-Garantie: für eine gegebene IDL-Quelle bleibt der Codegen-Output API-kompatibel über Minor-Versions (Whitespace + Comment-Refactors sind erlaubt, Breaking-API-Changes nicht).

### Out-of-Scope (Phase 2+)

- IDL `bitset` / `bitmask` (Spec §7.4.7) — out-of-scope, da DDS-Topics sie nicht typisch verwenden.
- IDL `fixed` (Spec §7.4.4.5) — out-of-scope (financial-Domain-Bedarf nur über CORBA-Pfad).
- IDL `map<K, V>` (Spec §7.4.4.6) — out-of-scope (kein Rust-Standard-Mapping ohne BTreeMap-/HashMap-Wahl).
- IDL `any` (Spec §7.4.4.7) — out-of-scope (Type-Erasure passt nicht zur Rust-Generic-Strategie).
- IDL `valuetype` / `interface` / `component` / `home` — CORBA-Konstrukte, ausserhalb des DDS-DataType-Pfades.
- Mutable-Decode mit beliebiger Member-Reihenfolge (Phase 2 read_mutable_member-Loop) — aktuell decoded mutable als appendable in deklarations-Reihenfolge.
