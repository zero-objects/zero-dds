# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung.

### Spec-Referenzen
- `docs/specs/zerodds-xcdr2-rust-1.0.md` §11.1 — `#[derive(DdsType)]`
  als Folge-Sprint klassifiziert; in dieser Crate voll implementiert.
- OMG XTypes 1.3 §7.4 — Wire-Format der emittierten encode/decode-
  Methoden.
- OMG XTypes 1.3 §7.6.8.4 — Key-Hash-Berechnung (≤ 16 Bytes
  zero-pad, sonst MD5).

### Public-API
- [`DdsType`](src/lib.rs) — Proc-Macro-Derive-Attribut. Annotiert
  einen Plain-`struct` und emittiert eine `impl DdsType`-Block.
- Inner-Attribute:
  - `#[dds(type_name = "...")]` — explizite TYPE_NAME-Override.
  - `#[dds(key)]` — pro Member, markiert das Feld als `@key`.

### Implementierung
- AST-Walk via `syn 2`. `DeriveInput` -> `Data::Struct` ->
  `Fields::Named` Iteration.
- Pro-Field-Codegen via `quote::quote!`-Templates. Encode/decode
  delegiert an `zerodds_cdr::CdrEncode`/`CdrDecode`-Traits — keine
  Type-spezifische Match-Tabelle, jede primitive + composite Type
  in `zerodds_cdr` hat ein `impl CdrEncode/Decode`.
- Key-Hash-Pfad emittiert eine `encode_key_holder_be`-Override-
  Methode wenn mindestens ein `#[dds(key)]`-Member vorhanden ist.

Heute auf Final-Extensibility fokussiert (kein DHEADER, kein
EMHEADER). Appendable + Mutable bleiben dem `idl-rust`-Codegen
ueberlassen weil deren Logic auf Member-Granularitaet feiner ist
als das Macro praktikabel emittieren kann.

### Architektur
- Layer: 1 Primitives (Helper-Crate fuer `zerodds-cdr` und
  `zerodds-dcps`).
- Dependencies (in): `syn 2`, `quote 1`, `proc-macro2 1`.
- Dependents (out): User-Code, der `#[derive(DdsType)]` nutzt.
  Konsumiert die Trait-Implementations von `zerodds-cdr` und
  `zerodds-dcps` zur Compile-Zeit (transitive build-Dependency).
- Feature-Flags: keine.

### Stabilitaet
- Alle `pub`-Items sind RC1-stabil.
- Macro-Output-Form (genaues Token-Layout) ist NICHT stabil und kann
  zwischen Minor-Versionen aendern; semantisch bleibt `impl DdsType`-
  Spec-Form gleich.

[1.0.0-rc.1]: https://github.com/zero-objects/zero-dds/releases/tag/v1.0.0-rc.1
