# `zerodds-corba-rust`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

IDL → Rust code generator for **CORBA service constructs** (interface traits + stubs + skeletons, valuetypes; in phase 2: components, homes, POA bindings).

Analogous to `zerodds-idl-cpp` / `-csharp` / `-java` — but emits Rust instead of C++/C#/Java. Consumes `zerodds-corba-codegen` helpers and `zerodds-idl-rust::type_map`.

## Layer position

Layer 8 (CORBA stack). Build-time tool, std-only.

## What is emitted

| IDL                           | Rust                                              |
|-------------------------------|---------------------------------------------------|
| `interface I { op(...); };`   | `pub trait I` + `pub struct IStub` + `dispatch_i` |
| `attribute T x`               | trait getter + setter (if writable)               |
| `oneway op(...)`              | trait method without reply                        |
| `valuetype V { ... };`        | `pub trait V: ValueBase`                          |
| `module M { … }`              | `pub mod M { … }`                                 |

## Quickstart

```rust
use zerodds_idl::config::ParserConfig;
use zerodds_idl::features::IdlFeatures;
use zerodds_corba_rust::{generate_corba_rust_module, CorbaRustGenOptions};

let cfg = ParserConfig { features: IdlFeatures::corba_full(), ..Default::default() };
let ast = zerodds_idl::parse(
    "interface Calculator { long add(in long a, in long b); };",
    &cfg,
).expect("parse");
let rust_src = generate_corba_rust_module(&ast, &CorbaRustGenOptions::default()).expect("gen");
```

## License

Apache-2.0.
