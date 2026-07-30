# `zerodds-idl-rust`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-idl-rust/badge.svg)](https://docs.rs/zerodds-idl-rust)

IDL4 → Rust code generator for [ZeroDDS](https://zerodds.org) DataTypes.

Reads the IDL AST from `zerodds-idl` and emits Rust code with:
- `pub struct` / `pub enum` / `pub type`
- `impl zerodds_dcps::DdsType` (`encode`/`decode`/`encode_key_holder_be`)
- `impl zerodds_cdr::CdrEncode` / `CdrDecode` for enums

Complements the other language codegens `idl-cpp` / `idl-csharp` / `idl-java` / `idl-ts` with the Rust path — end users can use IDL-first workflows in Rust too.

## Layer position

Layer 3 (schema). Build-time tool, std-only, `forbid(unsafe_code)`.

## What is emitted

| IDL | Rust |
|-----|------|
| `struct` (final) | `pub struct` + `impl DdsType` with XCDR2 final wire |
| `struct` (`@appendable`) | with `zerodds_cdr::struct_enc::encode_appendable` |
| `struct` (`@mutable`) | with `zerodds_cdr::struct_enc::MutableStructEncoder` |
| `enum` | `pub enum #[repr(i32)]` + `from_wire` + `CdrEncode/Decode` |
| `union` | `pub enum` with variants per case |
| `typedef` | `pub type X = Y;` |
| `module` | `pub mod m { ... }` with nested definitions |
| `@key` | `encode_key_holder_be` implementation, member-id-sorted |
| `@id(N)` | member ID for mutable extensibility and KeyHolder sorting |

## Generated code dependencies

In default mode, the emitted Rust module's `[dependencies]` are exactly
`zerodds-cdr`, `zerodds-dcps`, `zerodds-types` — no direct dependency on
`zerodds-sql-filter` is needed. `field_value` uses `zerodds_dcps::FilterValue`
(the crate's re-export of the SQL-filter value type), the same convention
`zerodds-cdr-derive`'s `#[derive(DdsType)]` already uses. In `cdr_only` mode
(the CORBA/GIOP path, no `DdsType` impl) only `zerodds-cdr` is needed.

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

- **Snapshot tests** (`tests/snapshot_codegen.rs`) — 13 tests, each compares the emitted code against a committed `.snap` file.
- **Compile-check tests** (`tests/compile_check.rs`, `--include-ignored`) — 8 tests, each actually compiles the emitted code against a temp crate with path deps on `zerodds-cdr`+`zerodds-dcps`. Proves the output is not only snapshot-consistent but also really compilable.

```bash
cargo test -p zerodds-idl-rust --tests                                  # snapshot + smoke
cargo test -p zerodds-idl-rust --test compile_check -- --include-ignored # real-compile
```

## License

Apache-2.0. See [LICENSE](../../LICENSE).

## See also

- [`docs/architecture/02_architecture.md`](../../docs/architecture/02_architecture.md) — layer architecture
- [`crates/idl-cpp`](../idl-cpp/) — reference codegen (C++17 header)
- [`crates/idl-csharp`](../idl-csharp/) — C# P/Invoke codegen
- [`crates/idl-java`](../idl-java/) — Java JNI codegen
- [`crates/idl-ts`](../idl-ts/) — TypeScript codegen
