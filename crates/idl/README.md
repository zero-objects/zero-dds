# `zerodds-idl`

Grammar-driven Parser fuer **OMG IDL 4.2** (ISO/IEC 19516), mit
Vendor-Extensions-Pipeline fuer schmerzfreie Migration aus RTI Connext,
OpenSplice, Cyclone DDS und Fast-DDS.

Teil des Projekts [**ZeroDDS**](../../README.md). Safety-Klasse **SAFE
(std-only)** — forbid(unsafe_code), keine panic!/unwrap/expect in
Production-Code, deterministisch.

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

## Vendor-Extensions (RTI Connext)

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

Ohne `RTI_CONNEXT`-Delta wuerde die `keylist`-Direktive abgelehnt —
das ist Architektur-Design, kein Bug. Deltas sind additive Patches
auf die Base-Grammar; die Base bleibt Single-Source-of-Truth fuer
OMG-IDL-4.2.

## Mit Preprocessor

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
│            │    │ #include/  │    │  rules aus │    │  Recognize)│
│            │    │ #define/   │    │  Grammar)  │    │            │
│            │    │ #ifdef)    │    │            │    │            │
└────────────┘    └────────────┘    └────────────┘    └────────────┘
                                                             │
                                                             ▼
┌────────────┐    ┌────────────┐    ┌────────────┐    ┌────────────┐
│ Typed      │◀───│ AST-Builder│◀───│ CST        │◀───│ Parse-     │
│ AST        │    │ (CST→AST   │    │ (Memoized  │    │ Forest     │
│            │    │  mit       │    │  recon-    │    │ (Earley    │
│            │    │  Spans)    │    │  struction)│    │  state     │
│            │    │            │    │            │    │  sets)     │
└────────────┘    └────────────┘    └────────────┘    └────────────┘
      │
      ├──▶ `{ast}`-Pretty-Print (Roundtrip-faehig)
      └──▶ Backend Code-Gen — siehe `idl-cpp`, `idl-csharp`, `idl-java`, `idl-ts`
```

---

## Sales-Argument: Grammar-driven + Vendor-Deltas

Klassische IDL-Parser sind hand-geschriebene Recursive-Descent-Parser,
meist gefuellt mit Vendor-Hacks, um RTI/OpenSplice/Cyclone-Abweichungen
zu akzeptieren. Migration wird zum Refactoring-Marathon.

**Dieser Parser** nutzt:

1. **Eine zentrale OMG-IDL-4.2-Grammar** als `IDL_42`-Konstante
   (108 Productions, jede mit `spec_ref` auf den OMG-Spec-§).
2. **Earley-Engine** — akzeptiert beliebige kontextfreie Grammars
   (inkl. Linksrekursion), polynomial durch Memoization.
3. **Vendor-Deltas** als additive Patches: `GrammarDelta` fuegt
   Productions + Alternativen hinzu, ohne die Base zu aendern.

RTI-Connext-Delta: **100 LOC + 0 Hacks in Base-Grammar**. Sales-Pitch
fuer Migration:

> Ihr Code nutzt `#pragma keylist` oder `@rti::*`-Annotations?
> Unser Parser akzeptiert das ab Tag 1 — ohne Grammar-Fork, ohne
> Maintenance-Burden.

---

## Module-Struktur

| Modul | Zweck | Status |
|---|---|---|
| `lexer` | Token-Rules (auto-extrahiert aus Grammar), Comments, Literals |
| `grammar` | `IDL_42` + GrammarLike-Trait, Validate, Compile, Compose |
| `grammar::deltas` | RTI Connext / FastDDS / Cyclone DDS / OpenSplice Vendor-Deltas |
| `engine` | Earley-Recognizer |
| `cst` | Memoized CST-Builder |
| `ast` | Typed AST + Builder + Validator + Pretty-Print |
| `preprocessor` | `#include`/`#define`/`#ifdef` + SourceMap |
| `parser` | Top-Level `parse()` / `parse_with_deltas()` |
| `config` | `ParserConfig` (Version, CompatMode, VendorExt) |
| `validator` | Semantik-Validator: `@key`/`@id`/Inheritance/Annotation-Constraints |
| `xtypes` | TypeObject-Build + KeyHash + Assignability (XTypes 1.3) |

## Tests

```bash
cargo test -p zerodds-idl
```

Beinhaltet:

- **1000+ Lib-Tests** für Engine/Grammar/CST/AST/Preprocessor + Semantik-Validators
- **OMG-Fixtures**: `zerodds_dcps.idl`, `zerodds_security.idl`, `dds_xtypes.idl`
- **Vendor-Fixtures**: RTI Connext (5 E2E), Cyclone DDS (2), Fast-DDS (2)
- **Roundtrip**: parse → print → parse → AST-Aequivalenz (9 Tests)
- **Grammar-Coverage-Report** (`tests/coverage_report.rs` generiert
  `coverage_report.md`)

## CLI

```bash
cargo run -p zerodds-idlc -- --parse-only <file.idl>        # OMG IDL 4.2
cargo run -p zerodds-idlc -- --parse-only --rti <file.idl>  # + RTI-Delta
```

Code-Gen-Backends:

* `zerodds-idl-cpp` — C++17 (OMG IDL4-CPP-Mapping)
* `zerodds-idl-csharp` — C# 10 (OMG IDL4-CSharp-Mapping)
* `zerodds-idl-java` — Java 17 (OMG IDL4-Java-Mapping)
* `zerodds-idl-ts` — TypeScript (DDS-TS 1.0 Vendor-Spec)
* Rust-Backend in `zerodds-idlc` direkt integriert

---

## Spec-Audit-Stand

K1 IDL-Spec-Vervollstaendigung abgeschlossen 2026-04-28: alle 19
S-Res-Followup-Items live mit Builder + Validator + Tests; voll
spec-konform.

XTypes 1.3 §7.2.2.4.8 `@verbatim`: Code-Gen-Hook in C++/C#/Java
live, alle 6 PlacementKinds.

Legacy-IDL-Konstrukte (bitset, bitmask, fixed, any, valuetype,
non-service-interface) voll abgedeckt in cpp/csharp/java —
K10/K11/K12 = 100 % Spec-Coverage (57/65/71 done).

Vendor-Deltas:

| Vendor | Delta-Datei | Coverage |
|---|---|---|
| RTI Connext | `RTI_CONNEXT` | 100 LOC, alle major Pragmas |
| FastDDS | `FASTDDS` | XTypes-aliasing-Quirks |
| Cyclone DDS | `CYCLONEDDS` | Standard-OMG-IDL, kein Delta noetig |
| OpenSplice / TAO | `TAO_OPENSPLICE` | `#pragma DCPS_DATA_*` |

## Documentation

Fuer User-Guide siehe
[Documentation Trail Station 04 → IDL](../../documentation/04-idl/README.md)
mit Language-Reference + Annotations + Codegen-CLI.
