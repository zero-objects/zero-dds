# `zerodds-idl`

Grammar-driven parser for **OMG IDL 4.2** (ISO/IEC 19516), with a
vendor-extensions pipeline for painless migration from RTI Connext,
OpenSplice, Cyclone DDS and Fast-DDS.

Part of the [**ZeroDDS**](../../README.md) project. Safety class **SAFE
(std-only)** — forbid(unsafe_code), no panic!/unwrap/expect in
production code, deterministic.

---

## Quick Start

```rust
use zerodds_idl::{parse, config::ParserConfig};

let src = r#"
    @topic
    @appendable
    struct SensorReading {
        @key long sensor_id;
        double value;
    };
"#;

let ast = parse(src, &ParserConfig::default())?;
println!("{ast}");
# Ok::<(), zerodds_idl::Error>(())
```

## Vendor Extensions (RTI Connext)

```rust
use zerodds_idl::{config::ParserConfig, grammar::deltas::RTI_CONNEXT,
              parser::parse_with_deltas};

let src = r"
    struct Sensor { long id; double value; };
    keylist Sensor (id);
";
let ast = parse_with_deltas(src, &ParserConfig::default(), &[&RTI_CONNEXT])?;
# Ok::<(), zerodds_idl::Error>(())
```

Without the `RTI_CONNEXT` delta the `keylist` directive would be rejected —
that is by architectural design, not a bug. Deltas are additive patches
on top of the base grammar; the base remains the single source of truth for
OMG IDL 4.2.

## With Preprocessor

```rust
use zerodds_idl::preprocessor::{Preprocessor, MemoryResolver};
use zerodds_idl::parser::parse;
use zerodds_idl::config::ParserConfig;

let mut resolver = MemoryResolver::new();
resolver.add("common.idl", "struct Header { long seq_num; };");

let pp = Preprocessor::new(resolver);
let processed = pp.process("main.idl", r#"
    #include "common.idl"
    #define MAX 100
    struct Payload { long limit; };
"#)?;

let ast = parse(&processed.expanded, &ParserConfig::default())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

---

## Pipeline

```text
┌────────────┐    ┌────────────┐    ┌────────────┐    ┌────────────┐
│ Source     │───▶│ Preproc.   │───▶│ Lexer      │───▶│ Engine     │
│ (*.idl)    │    │ (optional) │    │ (token     │    │ (Earley    │
│            │    │ #include/  │    │  rules from│    │  Recognize)│
│            │    │ #define/   │    │  grammar)  │    │            │
│            │    │ #ifdef)    │    │            │    │            │
└────────────┘    └────────────┘    └────────────┘    └────────────┘
                                                             │
                                                             ▼
┌────────────┐    ┌────────────┐    ┌────────────┐    ┌────────────┐
│ Typed      │◀───│ AST Builder│◀───│ CST        │◀───│ Parse      │
│ AST        │    │ (CST→AST   │    │ (Memoized  │    │ Forest     │
│            │    │  with      │    │  recon-    │    │ (Earley    │
│            │    │  spans)    │    │  struction)│    │  state     │
│            │    │            │    │            │    │  sets)     │
└────────────┘    └────────────┘    └────────────┘    └────────────┘
      │
      ├──▶ `{ast}` pretty-print (roundtrip-capable)
      └──▶ Backend code-gen — see `idl-cpp`, `idl-csharp`, `idl-java`, `idl-ts`
```

---

## Sales Argument: Grammar-driven + Vendor Deltas

Classic IDL parsers are hand-written recursive-descent parsers,
mostly stuffed with vendor hacks to accept RTI/OpenSplice/Cyclone
deviations. Migration turns into a refactoring marathon.

**This parser** uses:

1. **A single central OMG IDL 4.2 grammar** as the `IDL_42` constant
   (108 productions, each with a `spec_ref` to the OMG spec §).
2. **An Earley engine** — accepts arbitrary context-free grammars
   (incl. left recursion), polynomial via memoization.
3. **Vendor deltas** as additive patches: `GrammarDelta` adds
   productions + alternatives without changing the base.

RTI Connext delta: **100 LOC + 0 hacks in the base grammar**. Sales pitch
for migration:

> Your code uses `#pragma keylist` or `@rti::*` annotations?
> Our parser accepts that from day 1 — no grammar fork, no
> maintenance burden.

---

## Module Structure

| Module | Purpose | Status |
|---|---|---|
| `lexer` | Token rules (auto-extracted from grammar), comments, literals |
| `grammar` | `IDL_42` + GrammarLike trait, validate, compile, compose |
| `grammar::deltas` | RTI Connext / FastDDS / Cyclone DDS / OpenSplice vendor deltas |
| `engine` | Earley recognizer |
| `cst` | Memoized CST builder |
| `ast` | Typed AST + builder + validator + pretty-print |
| `preprocessor` | `#include`/`#define`/`#ifdef` + SourceMap |
| `parser` | Top-level `parse()` / `parse_with_deltas()` |
| `config` | `ParserConfig` (version, CompatMode, VendorExt) |
| `validator` | Semantic validator: `@key`/`@id`/inheritance/annotation constraints |
| `xtypes` | TypeObject build + KeyHash + assignability (XTypes 1.3) |

## Tests

```bash
cargo test -p zerodds-idl
```

Includes:

- **1000+ lib tests** for engine/grammar/CST/AST/preprocessor + semantic validators
- **OMG fixtures**: `zerodds_dcps.idl`, `zerodds_security.idl`, `dds_xtypes.idl`
- **Vendor fixtures**: RTI Connext (5 E2E), Cyclone DDS (2), Fast-DDS (2)
- **Roundtrip**: parse → print → parse → AST equivalence (9 tests)
- **Grammar coverage report** (`tests/coverage_report.rs` generates
  `coverage_report.md`)

## CLI

```bash
cargo run -p zerodds-idlc -- --parse-only <file.idl>        # OMG IDL 4.2
cargo run -p zerodds-idlc -- --parse-only --rti <file.idl>  # + RTI delta
```

Code-gen backends:

* `zerodds-idl-cpp` — C++17 (OMG IDL4 C++ mapping)
* `zerodds-idl-csharp` — C# 10 (OMG IDL4 C# mapping)
* `zerodds-idl-java` — Java 17 (OMG IDL4 Java mapping)
* `zerodds-idl-ts` — TypeScript (DDS-TS 1.0 vendor spec)
* Rust backend integrated directly into `zerodds-idlc`

---

## Spec Audit Status

K1 IDL spec completion finished 2026-04-28: all 19
S-Res follow-up items live with builder + validator + tests; fully
spec-compliant.

XTypes 1.3 §7.2.2.4.8 `@verbatim`: code-gen hook in C++/C#/Java
live, all 6 PlacementKinds.

Legacy IDL constructs (bitset, bitmask, fixed, any, valuetype,
non-service-interface) fully covered in cpp/csharp/java —
K10/K11/K12 = 100 % spec coverage (57/65/71 done).

Vendor deltas:

| Vendor | Delta file | Coverage |
|---|---|---|
| RTI Connext | `RTI_CONNEXT` | 100 LOC, all major pragmas |
| FastDDS | `FASTDDS` | XTypes aliasing quirks |
| Cyclone DDS | `CYCLONEDDS` | Standard OMG IDL, no delta needed |
| OpenSplice / TAO | `TAO_OPENSPLICE` | `#pragma DCPS_DATA_*` |

## Documentation

For the user guide see
[Documentation Trail Station 04 → IDL](../../documentation/04-idl/README.md)
with language reference + annotations + codegen CLI.
