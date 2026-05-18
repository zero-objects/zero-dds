# `zerodds-idl-cpp`

IDL4 → **C++17- und C-Header-Codegen** fuer ZeroDDS (OMG IDL4-CPP
formal/2018-07-01 + DDS-PSM-CXX 1.0 + DDS-RPC C++ PSM). Liefert
Standalone-Headers, die ueber `zerodds-c-api` an die Runtime andocken.

Teil des Projekts [**ZeroDDS**](../../README.md). Safety-Klasse
**SAFE (std-only)** — `forbid(unsafe_code)`, Build-Zeit-Tool ohne
no_std-Use-Case.

---

## Quick Start — C++

```rust
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

let ast = zerodds_idl::parse(
    "module M { struct S { long x; }; };",
    &ParserConfig::default(),
)?;

let header = generate_cpp_header(&ast, &CppGenOptions::default())?;
assert!(header.contains("namespace M"));
assert!(header.contains("class S"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Quick Start — reines C

```rust
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CGenOptions, generate_c_header};

let ast = zerodds_idl::parse(
    "struct Greeting { long id; string<128> text; };",
    &ParserConfig::default(),
)?;

let c_header = generate_c_header(&ast, &CGenOptions::default())?;
assert!(c_header.contains("Greeting"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Im CLI uebernimmt das `zerodds-idlc` Tool die beiden Pfade als
`--cpp` bzw. `--c` Backend. Output landet als `<basename>.hpp`
beziehungsweise `<basename>.h`.

## Scope

| Block | Was wird emittiert | Spec |
| --- | --- | --- |
| C5.1-a | Header-Layout, Primitive-Mapping, struct/enum/union/typedef/sequence/array/inheritance, Exception | IDL4-CPP §7 |
| C5.1-b | Status-Klassen (13), QoS-Policies (22), DCPS-Entity-Header-Stubs | DCPS §7, §8 |
| C5.2 | DDS-PSM-CXX-Header-Skeleton-Layer | DDS-PSM-CXX 1.0 |
| C6.1.D-cpp | DDS-RPC C++ PSM: Service-Interface, Requester, Replier, RemoteException-Hierarchie | DDS-RPC 1.0 §10 |
| C-Mode | Reine C-Header mit ZeroDDS-Konventionen | `c_mode.rs` |

## Spec-Mapping

| Spec-Dokument | Abschnitt |
| --- | --- |
| OMG IDL 4.2 (ISO/IEC 19516) | §7 — Konstrukt-Mapping |
| OMG IDL4-CPP 1.0 | §7 — Header-Layout |
| OMG DDS-PSM-CXX 1.0 | §3-§5 — Entity-API |
| OMG DDS-RPC 1.0 | §10 — C++ PSM |
| OMG DDS-XTypes 1.3 | §7.2.3 — Annotations + Extensibility |

## Bewusst NICHT im Crate

- **Bitset/Bitmask, Map, Fixed, Any, Interface, Valuetype** — Phase-2-Material.
- **Linker-Tests** — statische Header-Generation reicht; Roundtrip-Tests laufen in `crates/cpp/tests/`.

## Features

* `default = []` — std-only, kein Feature noetig.

## Stabilitaet

`1.0.0-rc.2` — Wire-byte-identisch zu Cyclone DDS / RTI Connext /
Fast-DDS. API kann bis 1.0.0-final noch in Detail-Punkten brechen
(Options-Field-Reihenfolge, Error-Varianten); generierte Header bleiben
ABI-stabil.

## Tests

```bash
cargo test -p zerodds-idl-cpp
```

Fixture-IDLs unter `tests/fixtures/`, Snapshot-Tests pro Konstrukt.

## See also

- [`zerodds-idl`](../idl/README.md) — Parser + AST (Input-Seite).
- [`zerodds-cpp`](../cpp/README.md) — C++17-RAII-Wrapper, Runtime-Seite des Bindings.
- [`zerodds-c-api`](../zerodds-c-api/README.md) — C-FFI, das die Header gegen die Rust-Runtime anbindet.
- [`zerodds-idlc`](../../tools/idlc/README.md) — CLI mit `--cpp` und `--c` Flag.
- [`packaging/docker/cpp-runtime/`](../../packaging/docker/cpp-runtime/) — Sandbox-Image mit Toolchain + Headern.
