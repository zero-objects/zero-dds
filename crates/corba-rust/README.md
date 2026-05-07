# `zerodds-corba-rust`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

IDL → Rust Code-Generator für **CORBA-Service-Konstrukte** (Interface-Traits + Stubs + Skeletons, Valuetypes, in Phase-2: Components, Homes, POA-Bindings).

Analog zu `zerodds-idl-cpp` / `-csharp` / `-java` — aber emittiert Rust statt C++/C#/Java. Konsumiert `zerodds-corba-codegen`-Helpers und `zerodds-idl-rust::type_map`.

## Schichten-Position

Layer 8 (CORBA-Stack). Build-Zeit-Tool, std-only.

## Was emittiert wird

| IDL                           | Rust                                              |
|-------------------------------|---------------------------------------------------|
| `interface I { op(...); };`   | `pub trait I` + `pub struct IStub` + `dispatch_i` |
| `attribute T x`               | trait getter + setter (wenn writable)             |
| `oneway op(...)`              | trait method ohne Reply                           |
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

## Lizenz

Apache-2.0.
