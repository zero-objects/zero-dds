# dds-XXX

> One-Line-Beschreibung der Crate.

[![Crates.io](https://img.shields.io/crates/v/dds-XXX.svg)](https://crates.io/crates/dds-XXX)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

ZeroDDS-Komponente: kurze Architektur-Einordnung (Foundation, Protocol,
Transport, etc.).

## Spec-Mapping

| Spec-Dokument | Abschnitt |
| ------------- | --------- |
| OMG DDS 1.4   | §X.Y.Z    |
| OMG DDSI-RTPS 2.5 | §A.B  |

## Safety-Klassifikation

**SAFE** / **STANDARD** / **COMFORT** — siehe
`docs/architecture/04_safety_by_architecture.md`.

## Verwendung

```rust
use dds_XXX::Foo;
let f = Foo::new();
```

## Features

* `default = ["std"]` — Standard-Library + Heap-Allocator.
* `std` — bevorzugte Variante.
* `alloc` — no_std + Heap.
* `safety` — extra Defensive-Checks.

## Stabilitaet

`0.1.x` — Pre-1.0 API, Breakage moeglich.

## Build & Test

```bash
cargo build -p dds-XXX
cargo test -p dds-XXX
```

## Lizenz

Apache-2.0. Siehe [LICENSE](../LICENSE).
