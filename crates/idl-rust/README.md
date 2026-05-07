# `zerodds-idl-rust`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-idl-rust/badge.svg)](https://docs.rs/zerodds-idl-rust)

IDL4 → Rust Code-Generator für [ZeroDDS](https://zerodds.org)-DataTypes.

Liest den IDL-AST aus `zerodds-idl` und emittiert Rust-Code mit:
- `pub struct` / `pub enum` / `pub type`
- `impl zerodds_dcps::DdsType` (`encode`/`decode`/`encode_key_holder_be`)
- `impl zerodds_cdr::CdrEncode` / `CdrDecode` für Enums

Ergänzt die anderen Sprach-Codegens `idl-cpp` / `idl-csharp` / `idl-java` / `idl-ts` um den Rust-Pfad — End-User können IDL-First-Workflows auch in Rust nutzen.

## Schichten-Position

Layer 3 (Schema). Build-Zeit-Tool, std-only, `forbid(unsafe_code)`.

## Was wird emittiert

| IDL | Rust |
|-----|------|
| `struct` (final) | `pub struct` + `impl DdsType` mit XCDR2-Final-Wire |
| `struct` (`@appendable`) | mit `zerodds_cdr::struct_enc::encode_appendable` |
| `struct` (`@mutable`) | mit `zerodds_cdr::struct_enc::MutableStructEncoder` |
| `enum` | `pub enum #[repr(i32)]` + `from_wire` + `CdrEncode/Decode` |
| `union` | `pub enum` mit Variants pro Case |
| `typedef` | `pub type X = Y;` |
| `module` | `pub mod m { ... }` mit nested Definitions |
| `@key` | `encode_key_holder_be` Implementation, member-id-sortiert |
| `@id(N)` | Member-ID für mutable extensibility und KeyHolder-Sortierung |

## Quickstart

```rust
use zerodds_idl::config::ParserConfig;
use zerodds_idl_rust::{generate_rust_module, RustGenOptions};

let ast = zerodds_idl::parse(
    "@appendable struct Telemetry { unsigned long ts; double v; };",
    &ParserConfig::default(),
).expect("parse");

let rust_src = generate_rust_module(&ast, &RustGenOptions::default()).expect("gen");
println!("{rust_src}");
```

## Tests

- **Snapshot-Tests** (`tests/snapshot_codegen.rs`) — 13 Tests, jeder vergleicht den emittierten Code gegen einen committed `.snap`-File.
- **Compile-Check-Tests** (`tests/compile_check.rs`, `--include-ignored`) — 8 Tests, jeder kompiliert den emittierten Code tatsächlich gegen ein temp-Crate mit Pfad-Deps auf `zerodds-cdr`+`zerodds-dcps`. Belegt dass der Output nicht nur snapshot-konsistent sondern auch real-kompilierbar ist.

```bash
cargo test -p zerodds-idl-rust --tests                                  # snapshot + smoke
cargo test -p zerodds-idl-rust --test compile_check -- --include-ignored # real-compile
```

## Lizenz

Apache-2.0. Siehe [LICENSE](../../LICENSE).

## Siehe auch

- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — Schichten-Architektur
- [`crates/idl-cpp`](../idl-cpp/) — Vorbild-Codegen (C++17 Header)
- [`crates/idl-csharp`](../idl-csharp/) — C# P/Invoke Codegen
- [`crates/idl-java`](../idl-java/) — Java JNI Codegen
- [`crates/idl-ts`](../idl-ts/) — TypeScript Codegen
