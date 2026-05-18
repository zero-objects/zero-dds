# RC1 Review — `zerodds-idl`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 3.1 (Schema — IDL Parser)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

OMG IDL 4.2 (ISO/IEC 19516:2020) Parser + AST + Semantik-Modell für
ZeroDDS. Earley-Recognizer auf zentraler Grammar (108 Productions mit
`spec_ref`-Annotationen), Memoization-Pass für polynomial-Laufzeit,
CST→AST-Builder mit Source-Spans, Vendor-Deltas als additive Patches.

## 2 Public-Strategy

🌐 public — Library für End-User-IDL-Parsing + Konsum durch
`idl-{cpp,csharp,java,rust,ts}`-Codegen-Crates.

## 3 Content-Inventur

### 3.1 Module

```
src/
├── lib.rs              # Crate-Header + Re-Exports
├── config.rs           # ParserConfig (Version, CompatMode, VendorExt)
├── parser.rs           # Top-Level parse() / parse_with_deltas()
├── errors.rs           # Error-Family
├── lexer/              # Token-Rules (auto-extrahiert aus Grammar)
├── grammar/            # IDL_42 + GrammarLike + Compose + Compile + Validate + Vendor-Deltas
├── engine/             # Earley-Recognizer + Memoization
├── cst/                # Memoized CST-Builder
├── ast/                # Typed AST (types, builder, print)
├── semantics/          # Spec-Validators + Annotations + Const-Evaluation
├── preprocessor/       # #include/#define/#ifdef + SourceMap
└── features/           # Feature-Gate-Logic (DDS-CCM, etc.)
```

42 src-files, ~36 KLOC, 604 Public-Items, 1047 Tests.

### 3.2 Public-API-Surface

Aufgeschlüsselt nach Family in §3.4. Top-Level: `parse`,
`parse_with_deltas`, `ParserConfig`, `IdlAst`, `Builder`, `Validator`,
`Preprocessor`, `IDL_42`, plus alle AST-Type-Definitionen
(`StructDef`, `UnionDef`, `EnumDef`, `Annotation`, `MemberKind`, …).

### 3.3 Tests

- `cargo test -p zerodds-idl --lib`: ✅ 1047 passed.
- OMG-Fixtures: `dds_dcps.idl`, `dds_security.idl`, `dds_xtypes.idl`.
- Vendor-Fixtures: RTI Connext (5 E2E), Cyclone DDS (2), Fast-DDS (2).
- Roundtrip-Tests (parse → print → parse → AST-Aequivalenz, 9 Tests).
- Grammar-Coverage-Report (`tests/coverage_report.rs` generiert
  `coverage_report.md`).

### 3.4 Coherence-Audit (§1.5b) — gruppiert nach Public-API-Family

604 Public-Items gesamt, gruppiert in 8 Families:

| Family | Items (sample) | Spec-Anker | External Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| **Top-Level Parser-API** | `parse`, `parse_with_deltas`, `ParserConfig`, `Error`, `Result` | OMG IDL 4.2 §7.4 (Parser-Entry) | massiv (idl-cpp, idl-csharp, idl-java, idl-rust, idl-ts, tools/idlc) | CONNECTED | — |
| **AST-Types** (~150 Items) | `StructDef`, `UnionDef`, `EnumDef`, `BitmaskDef`, `BitsetDef`, `TypedefDef`, `Annotation`, `MemberKind`, `TypeSpec`, `PrimitiveType`, `IntegerType`, `FloatingType`, `Declarator`, `Member`, `ConstExpr`, `Literal`, `Span`, `Identifier`, `IdlAst`, `Module`, `InterfaceDef`, `ValueDef`, `ComponentDef`, `Annotation*`-Sub-Types, `OpDcl`, `AttrDcl`, ... | OMG IDL 4.2 §7.4.1-§7.4.18 (alle Building-Blocks) | massiv (alle 5 Codegen-Crates konsumieren AST) | CONNECTED + SPEC-MANDATED | — |
| **Builder + Validator** | `Builder`, `Validator`, Builder-Submethods | OMG IDL 4.2 §7.4 (programmatische AST-Konstruktion) | idl-rust, Tests | CONNECTED | — |
| **Lexer** | `Lexer`, `Token`, `TokenKind`, `LexError` | OMG IDL 4.2 §7.2 (Lexical Conventions) | grammar-engine + tests | CONNECTED | — |
| **Grammar-Engine** | `IDL_42`, `GrammarLike`, `Production`, `Alternative`, `RuleId`, `RuleRef`, `Symbol`, `Repeat`, `Sep`, `LiteralKind`, `GrammarDelta`, `Compiler`, `Composer`, `Validator`, Vendor-Deltas (`RTI_CONNEXT`, `FASTDDS`, `CYCLONEDDS`, `TAO_OPENSPLICE`) | OMG IDL 4.2 §7.4 (Grammar) + Vendor-Spec-Erweiterungen | engine + tools | CONNECTED + SPEC-MANDATED | — |
| **Earley-Engine** | `Recognizer`, `EarleyState`, `EarleyChart`, `RecognizerStats` | (interne Earley-Implementation) | grammar/cst | CONNECTED | — |
| **CST-Builder** | `Cst`, `CstNode`, `CstBuilder`, `CstError` | (interne CST-Repräsentation) | ast | CONNECTED | — |
| **Semantik-Validators** | `spec_validators::*` (~30 Validator-Functions), `annotations::*` (Built-in-Annotations: `@key`, `@id`, `@optional`, `@must_understand`, `@nested`, `@final`, `@appendable`, `@mutable`, `@bit_bound`, `@default`, `@external`, `@autoid`, `@verbatim`, ...), `const_eval::*` (Const-Expression-Evaluation pro IDL-§7.4.4 Constants) | OMG IDL 4.2 §7.4.4 + XTypes 1.3 §8.3 (Built-in-Annotations) + IDL §7.4.5 (Const-Eval) | semantik-Tests + Build/Validator | CONNECTED + SPEC-MANDATED | — |
| **Preprocessor** | `Preprocessor`, `MemoryResolver`, `FileResolver`, `IncludeResolver`-Trait, `SourceMap`, `PpError`, `PpResult`, `Macro`, `MacroDef` | OMG IDL 4.2 §7.3 (Preprocessing C-style) | parser + tests | CONNECTED | — |
| **Features-Gate** | `Features`, `FeatureSet`, `FeatureFlag` | (interne Profile-Logic für DDS-CCM-Erweiterungen) | parser + tests | CONNECTED | — |
| **Const-Eval-Helpers (15 DOC-ONLY)** | `cast_int8`, `cast_octet`, `cast_short`, `cast_uint8`, `cast_ulong`, `cast_ushort`, `check_const_decl_type_match`, `concat_strings`, `concat_wstrings`, `ConstValue`, ... | OMG IDL 4.2 §7.4.4 (Konstanten-Auswertung) | 0 ext direkt; via `const_eval::evaluate` indirekt | VENDOR-EXTENSION (Public-Helper-API für End-User-Custom-Const-Evaluators) | doc-as-hook |

**Zusammenfassung:** 604/604 Public-Items klassifiziert. **0 DEAD.**

Bei einer Parser-Library ist es per Definition normal, dass die meisten
AST-/Grammar-Items als Public exposed sind — Konsumenten (Codegen-
Backends, End-User-Tooling) müssen vollständigen Zugriff auf die AST-
Hierarchie haben. Die "OVER-EXPOSED"-Klassifikation aus dem Roh-Sweep
ist daher in den meisten Fällen unzutreffend — das sind SPEC-MANDATED
Public-API-Items.

## 4 Wiring

### 4.1 Dependencies

```toml
zerodds-types = { path = "../types", default-features = false, features = ["alloc", "std"] }
num-bigint = "0.4"   # IDL 4.2 §7.4.1.4.3 Fixed-Point (62-Digit Zwischenergebnis)
num-traits = "0.2"
```

### 4.2 Dependents

5 Codegen-Crates: `zerodds-idl-cpp`, `zerodds-idl-csharp`,
`zerodds-idl-java`, `zerodds-idl-rust`, `zerodds-idl-ts`. Plus
`tools/idlc` (CLI).

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std-only Crate (Build-Zeit-Tool) |

## 5 Spec-Relevanz

- **Spec(s):** OMG IDL 4.2 (formal/2018-01-05) / ISO/IEC 19516:2020.
  Plus XTypes 1.3 §7.3.1.2 (NameHash) und §8.3 (Built-in-Annotations).
- **Coverage-Doc:** `docs/spec-coverage/idl-4.2.md` (Spec-Audit
  K1 IDL-Spec-Vervollständigung 2026-04-28: alle 19 S-Res-Followup-
  Items live).

## 6 Cleanup-Findings

### 6.1 Forbidden-Token-Sweep

Treffer: keine.

### 6.2 Soft-Review (TODO/FIXME/HACK)

Treffer: keine.

### 6.3 Phase-Marker (§1.13)

20 Phase-Marker in den ersten Sweeps gefunden:
- `cdr/encode.rs` Verweis (Layer-1) — false positive (idl referenziert
  cdr in Doc-Comments)
- `engine/recognize.rs:126/547/570` — "Phase-0-Limitierung"
- `parser.rs:268` — "Phase-0: ungenutzt"
- `config.rs:201/203` — "Phase-1-Stand", "Phase-2"
- `grammar/validate.rs:187` — "Phase-0-Scope"
- `grammar/compose.rs:12` — "Spike-Phase-0-Regel"
- `cst/build.rs:34/62` — "Engine-Phase-0-Limitierung"
- `ast/print.rs:116` — "Phase-3-Code-Gen-Material"
- `ast/types.rs:445` — "Phase-0-AST-Builder"
- `preprocessor/mod.rs:3/58/544/550/695/735` — diverse Phase-X
- `semantics/spec_validators.rs:993` — "Phase-2-Verfeinerung"
- `semantics/annotations.rs:197` — "Phase-2"
- `grammar/idl42.rs:1452/4570/4577/4602-4608/6234/6345/6362` — diverse

Alle durch fachliche Beschreibung ersetzt (Bulk-Sweep + manuelle
Korrekturen).

### 6.4 Tech-Debt + Dead Code

Keine. 0 DEAD nach §1.5b-Sweep.

### 6.5 Public-API-Leaks

Keine — `pub use`-Re-Exports in `lib.rs` listen alle Items explizit.

## 7 Cleanup-Actions

1. SPDX-Header in 42 src-Files.
2. Cargo.toml RC1-Metadata (homepage, documentation, keywords, categories).
3. `publish = false` → `publish = true` (idl ist End-User-Library).
4. ~25 Phase-X-Marker rewriting (Bulk-Sweep + manuelle Korrekturen).
5. README test-count auf "1000+" aktualisiert.
6. CHANGELOG.md erstellt.

## 8 Spec-Doc-Updates

`docs/spec-coverage/idl-4.2.md` bereits done (K1 IDL-Spec-Audit 2026-04-28).

## 9 Doc-Artefacts

- [x] Cargo.toml RC1
- [x] lib.rs-Header (bereits voll)
- [x] README + CHANGELOG
- [x] doc-Examples in README

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-idl --lib                              # ✅ 1047 passed
cargo clippy -p zerodds-idl --all-targets -- -D warnings     # ✅
cargo doc -p zerodds-idl --no-deps                           # ✅
```

## 11 RC1-DoD-Checkliste

- [x] §1.1-§1.13 alle ✅

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
