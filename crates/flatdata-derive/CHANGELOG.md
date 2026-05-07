# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-06

Initiale Release-Materialisierung der `zerodds-flatdata-derive`-Crate.

### Spec-Referenzen

- **`docs/specs/zerodds-flatdata-1.0.md`** §1.2 (Derive-Macro): `#[derive(FlatStruct)]` generiert `unsafe impl FlatStruct for T` mit `TYPE_HASH = sha256(layout_signature(T))[..16]`.

### Public-API

- `#[derive(FlatStruct)]` — proc-macro-derive auf `zerodds_flatdata::FlatStruct`. Generiert eine `unsafe impl`-Block mit der `TYPE_HASH`-Konstanten.

Es gibt keine weiteren oeffentlichen Items: die Crate ist eine reine `proc-macro = true`-Lib.

### Implementierung

Die `expand`-Funktion akzeptiert nur `struct`-DeriveInputs (Named, Tuple, Unit). `enum` und `union` werden mit `compile_error!` abgelehnt, weil ihr Layout selbst unter `repr(C)` nicht in derselben Form wie struct-Layouts vorhersagbar ist (Discriminant-Position, untagged-Union-Aliasing).

Vor der Hash-Berechnung wird `has_repr_c_or_transparent(&input.attrs)` geprueft. Fehlt `#[repr(C)]` oder `#[repr(transparent)]`, lehnt der Macro mit `compile_error!` ab — die Doc-Promise "FlatStruct verlangt repr(C)" wird damit zur Compile-Time-Garantie statt zu einem Caller-Vertrauensdokument.

Der `TYPE_HASH` ergibt sich aus `sha256(<TypeName>{<field-name>:<ty>,...})[..16]`. Der Layout-String enthaelt explizit:
- Type-Name (Type-Rename → neuer Hash).
- Field-Order (Field-Reorder → neuer Hash).
- Field-Names (bei Named-Structs) und Field-Type-Strings (Field-add/remove + Field-Type-Change → neuer Hash).

Die Bounds `Copy + 'static + Send + Sync` werden vom Trait selbst erzwungen — der Macro generiert keine eigenen Bound-Checks. Damit erscheinen Trait-Bound-Fehler als verstaendliche `T: Copy` /usw.-Compiler-Errors am Use-Site, nicht als kryptische Macro-Errors.

Generierte Code-Form:
```rust
#[automatically_derived]
unsafe impl ::zerodds_flatdata::FlatStruct for #name {
    const TYPE_HASH: [u8; 16] = [/* 16 byte */];
}
```

`WIRE_SIZE` und die `as_bytes`/`from_bytes_unchecked`-Methoden kommen als Trait-Default aus `zerodds-flatdata` und brauchen keinen Macro-Override.

### Architektur

- **Layer:** 4 (Core Services).
- **Dependencies (in):** `syn 2`, `quote 1`, `proc-macro2 1`, `sha2` (workspace, default-features=false). Keine ZeroDDS-Crate-Deps.
- **Dependents (out):** `zerodds-flatdata` (`dev-dependencies`, fuer den `tests/derive.rs`-Smoketest); end-user-Crates die FlatStruct ableiten wollen.
- **Feature-Flags:** keine.

### Stabilitaet

Alle oeffentlichen Macro-Pfade sind RC1-stabil. Das Layout-Signatur-Format ist Wire-stabil: eine Aenderung wuerde alle bisher generierten `TYPE_HASH`-Werte invalidieren und ist daher als Major-Breaking-Change klassifiziert.
