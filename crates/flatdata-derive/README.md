# `zerodds-flatdata-derive`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-flatdata-derive/badge.svg)](https://docs.rs/zerodds-flatdata-derive)

Proc-Macro `#[derive(FlatStruct)]` fuer den
[`zerodds-flatdata`](https://docs.rs/zerodds-flatdata)-Trait. Generiert
`unsafe impl FlatStruct for T` mit einem deterministischen
SHA-256-basierten `TYPE_HASH` aus Type-Name und Field-Layout. Safety
classification: **STANDARD** (proc-macro).

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| ZeroDDS-flatdata 1.0 | §1.2 (Derive-Macro) |

## Was ist drin

- **`#[derive(FlatStruct)]`** — generiert `unsafe impl ::zerodds_flatdata::FlatStruct`
  mit `TYPE_HASH = sha256(layout_signature(T))[..16]`.
- **Compile-Time-Checks**:
  - `enum`/`union` werden mit `compile_error!` abgelehnt (kein
    stabiles Layout).
  - Fehlendes `#[repr(C)]` / `#[repr(transparent)]` wird mit
    `compile_error!` abgelehnt (Default-Repr ist nicht garantiert
    layout-stabil).
- **Layout-Signatur** `<TypeName>{<field-name>:<field-ty>,...}` erkennt
  Type-Rename, Field-add/remove, Field-reorder, Field-Type-Change.

## Schichten-Position

Layer 4 — Core Services. Build-Time-Companion zu `zerodds-flatdata`,
keine Runtime-Code-Surface. Keine ZeroDDS-Crate-Deps.

## Quickstart

```rust,ignore
use zerodds_flatdata_derive::FlatStruct;

#[derive(Copy, Clone, FlatStruct)]
#[repr(C)]
struct Pose { x: f64, y: f64, z: f64 }

// expandiert zu:
//
//   unsafe impl ::zerodds_flatdata::FlatStruct for Pose {
//       const TYPE_HASH: [u8; 16] = [/* sha256("Pose{x:f64,y:f64,z:f64}")[..16] */];
//   }
```

`WIRE_SIZE` und die `as_bytes`/`from_bytes_unchecked`-Methoden kommen
als Default aus dem Trait und brauchen keinen Macro-Override.

## Feature-Flags

Keine. Die Crate ist eine reine `proc-macro`-Lib.

## Stabilitaet

`1.0.0-rc.1` ist die initiale Release-Materialisierung. Alle
oeffentlichen Macro-Pfade sind RC1-stabil; Layout-Signatur-Format gilt
als Wire-Stabil — eine Aenderung wuerde alle generierten `TYPE_HASH`-
Werte invalidieren und ist daher Major-Breaking.

## Build & Test

```bash
cargo build -p zerodds-flatdata-derive
# Tests stehen in zerodds-flatdata, weil proc-macro-Crates ihre eigenen
# Output nicht testen koennen:
cargo test -p zerodds-flatdata --test derive
```

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`zerodds-flatdata`](../flatdata) — der `FlatStruct`-Trait selbst.
- [`docs/specs/zerodds-flatdata-1.0.md`](../../docs/specs/zerodds-flatdata-1.0.md) §1.2.
