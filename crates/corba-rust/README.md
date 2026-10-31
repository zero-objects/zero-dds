# `zerodds-corba-rust`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

IDL → Rust code generator for **CORBA service constructs** (interface traits + stubs + skeletons, valuetypes, components, homes), plus the runtime types the generated code refers to (`ObjectReference`, `CorbaException`, `Servant`, POA builder).

It walks the common AST (`zerodds-ast`) and uses the Annex A.1 tables and the repository-id rule of `zerodds-corba-codegen`; the IDL → Rust type mapping is the crate's own `type_map` module. The data types (`struct`, `union`, `enum`) come from `zerodds-lang-rules`; this output is written to be concatenated with that one.

## Layer position

Layer 8 (CORBA stack). Build-time tool, std-only.

## What is emitted

| IDL                           | Rust                                              |
|-------------------------------|---------------------------------------------------|
| `interface I { op(...); };`   | `pub trait I` + `pub struct IStub` + `dispatch_i` |
| `attribute T x`               | trait getter + setter (if writable)               |
| `oneway op(...)`              | trait method without reply                        |
| `valuetype V { ... };`        | `pub trait V: ValueBase`                          |
| `component C`                 | `pub trait C: ComponentServant` + `C_REPOSITORY_ID` |
| `home H`                      | `pub trait H: ComponentHome` + `H_REPOSITORY_ID`  |
| `module M { … }`              | `pub mod M { … }`                                 |

## Quickstart

The generator takes a resolved model. Parse with `zerodds-idl-parse`, lower with `zerodds-idl-lower`, then resolve names:

```rust
use zerodds_corba_rust::{generate_corba_rust_module, CorbaRustGenOptions};

let idl = "interface Calculator { long add(in long a, in long b); };";
let parsed = zerodds_idl_parse::parse(idl);
assert!(parsed.diagnostics().iter().all(|d| !d.is_error()));
let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "calculator.idl");
assert!(lowered.diagnostics.iter().all(|d| !d.is_error()));
let mut ast = lowered.ast;
let report = ast.resolve_all_by_name().expect("resolve");
assert!(report.broken.is_empty());
let rust_src = generate_corba_rust_module(&ast, &CorbaRustGenOptions::default()).expect("gen");
```

## License

Apache-2.0.
