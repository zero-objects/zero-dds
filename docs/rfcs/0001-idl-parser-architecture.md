# RFC 0001 — IDL-Parser-Architektur (Grammar-driven, Earley-Engine)

| Feld | Wert |
|---|---|
| **Status** | **Accepted** (2026-04-18, post-Spike-Verifikation) |
| **Autor** | ZeroDDS Core Team (Claude-assistiert) |
| **Datum** | 2026-04-17 (Draft) → 2026-04-18 (Accepted) |
| **Review-Ziel** | Tech-Lead + Protocol-Owner |
| **Implementierungs-Anker** | `.planning/wp-0.3-idl-parser/PLAN.md` |
| **Crate** | `crates/idl` (Safe-klassifiziert, std-only) |
| **Roadmap-Zuordnung** | Phase 0 / WP 0.3 (siehe `06_roadmap.md §3`) — abgeschlossen |
| **Spike-Bilanz** | 442 lib + 5 RTI + 9 Roundtrip + 4 Vendor + Coverage-Report; 72% Grammar-Coverage; RTI-Delta als PoC validiert; `zerodds-idlc --parse-only` funktional |

## 1 Zusammenfassung

`zerodds-idl` implementiert einen **Parser-Parser-Ansatz** fuer OMG IDL 4.2: die
Grammatik liegt als Laufzeit-Daten vor, eine generische **Earley-Parse-Engine**
produziert aus Tokens einen untypisierten **Concrete Syntax Tree (CST)**, ein
nachgelagerter Builder baut daraus einen stark typisierten **Abstract Syntax
Tree (AST)**. Versions- und Vendor-Varianten werden als **Grammar-Deltas**
ueber eine Basis-Grammar ueberlagert.

Dieser Ansatz ist bewusst mehraufwendig als ein handgeschriebener Recursive-
Descent-Parser. Er ist strategisch motiviert: **ZeroDDS verkauft sich ueber
reibungsarme Migration aus bestehenden DDS-Stacks** (RTI Connext, eProsima
Fast DDS, Eclipse Cyclone DDS, OpenDDS, TwinOaks CoreDX). Vendor-spezifische
IDL-Dialekte und proprietaere Annotationen muessen ohne Refactoring
uebernehmbar sein. Ein Parser-Generator-Ansatz mit Grammar-Deltas ist der
technische Enabler dafuer.

## 2 Motivation

### 2.1 Strategische Motivation (primaer)

Aus `docs/architecture/07_risks_and_strategy.md §3.2`:

> Migration-Tools und Co-Existence-Modus (unser Stack und eProsima im selben
> Netzwerk) … Nicht auf „eProsima ersetzen" positionieren, sondern „naechste
> Generation von DDS".

Konkret: Kunden haben oft IDL-Bases von mehreren tausend Zeilen, die auf
Vendor-Dialekt-Features angewiesen sind (z.B. RTI `@transfer_mode`,
`@copy_declaration`; Cyclone `@idl-sequence-bound`-Konventionen; OpenDDS
`#pragma DCPS_DATA_TYPE`). Ein OMG-konformer Parser, der nichts davon
akzeptiert, fordert Kunden zu IDL-Refactoring auf — ein faktisches
Migrationshindernis.

**ZeroDDS-Verkaufszusage:** _„Ihr RTI-Connext-IDL-Corpus wird von uns
unveraendert verarbeitet."_ Das ist nur haltbar, wenn die Parser-
Architektur Vendor-Dialekte als First-Class-Buergern traegt, nicht als
Sonderfaelle.

### 2.2 Technische Motivation (sekundaer)

- **Spec-Updates (IDL 5.0, 6.0, …)** werden als Grammar-Daten-Patches
  absorbiert. Parser-Code bleibt unberuehrt. Review-Aufwand: diff der
  Grammar-Daten gegen Spec-BNF, statt Diff gegen handgeschriebenen
  Parser-Code. Das skaliert ueber 10–20 Jahre Produktlebensdauer.
- **Spec-zu-Code-Traceability**: jede Grammar-Production traegt ihre
  IDL-Spec-Section als Datenfeld (`spec_ref: "7.4.1.4.4.2"`). Das Tool
  `zerodds-traceability` aggregiert daraus automatisch eine Coverage-Matrix
  IDL-Section → Production → Test-Fixture → AST-Builder. Das ist
  audit-relevant in Expansion-Era (siehe `04_safety_by_architecture.md §6.3`).
- **Grammar-Ambiguity-Checks** als Eigentest: eine Grammar-Validation-
  Pass kann Productions auf Ambiguitaet, Linksrekursion und unbenutzte
  Nonterminals pruefen — analog zu `bison -v`.
- **Wiederverwendbarkeit**: die Lexer+Engine-Infrastruktur ist nicht
  IDL-spezifisch und kann spaeter fuer `zerodds-xml` (DDS-XML) oder sogar
  RTI-XML-QoS-Profile wiederverwendet werden.

## 3 Nicht-Ziele

- **Kein Code-Generator im Spike.** Code-Gen-Backends (C, C++, C#, Java,
  Python, Rust) leben in `tools/idlc` und sind Scope von WP 0.9 (nicht
  dieses RFC).
- **Keine Semantik-Analyse jenseits von AST-Bau.** Type-Checking,
  Cross-Reference-Resolution, @id-Kollisionen etc. kommen mit dem
  Code-Generator.
- **Kein no_std-Pfad.** `zerodds-idl` ist per Design std-only (Build-Zeit-
  Operation, kein embedded Use-Case). Diese Entscheidung wird in
  `02_architecture.md §3.3` festgehalten und in `crates/idl/src/lib.rs`
  dokumentiert.
- **Keine perfekte Conformance zur ISO/IEC 19516:2020** im Spike.
  OMG-IDL-4.2 ist inhaltsgleich (per `docs/standards/omg.md`), der Spike
  zielt auf OMG-Fassung.
- **Kein Linter / Formatter.** Beides kommt spaeter als separate Tools
  basierend auf dem CST.

## 4 Architektur-Uebersicht

### 4.1 Pipeline

```
Source-File (.idl)
    │
    ▼
┌─────────────────┐
│  Preprocessor   │  #include / #define / #ifdef / #pragma
│   (Stufe 0)     │  Output: praeprozessierter Token-Source-Text + Source-Map
└─────────────────┘
    │
    ▼
┌─────────────────┐
│  Generic Lexer  │  Token-Regeln aus Grammar-Daten extrahiert
│   (Stufe 1)     │  Output: TokenStream mit Source-Spans
└─────────────────┘
    │
    ▼
┌─────────────────┐
│  Earley Engine  │  Generischer Parser, Grammar-gesteuert
│   (Stufe 2)     │  Output: untypisierter CST mit Grammar-Production-Refs
└─────────────────┘
    │
    ▼
┌─────────────────┐
│  AST-Builder    │  CST → typed AST (ca. 60 Node-Typen)
│   (Stufe 3)     │  Output: ModuleSet (Root-AST)
└─────────────────┘
    │
    ▼
   Typed AST
(konsumiert von tools/idlc und zerodds-types)
```

### 4.2 Modul-Struktur

```
crates/idl/
├── Cargo.toml                   # std-only, kein alloc/safety-Feature
├── src/
│   ├── lib.rs                   # Public API: parse(source, config) -> Ast
│   ├── config.rs                # ParserConfig: IdlVersion, Compat, VendorExt
│   ├── grammar/
│   │   ├── mod.rs               # Daten-Typen: Grammar, Production, Rule, Symbol, SpecRef
│   │   ├── idl42.rs             # IDL 4.2 Base-Grammar (const static data)
│   │   ├── deltas/
│   │   │   ├── mod.rs
│   │   │   ├── idl40.rs         # Delta 4.0 ← 4.2 (Kompatibilitaets-Mode)
│   │   │   ├── rti_connext.rs   # RTI-Connext Vendor-Delta
│   │   │   ├── cyclonedx.rs     # Cyclone-DDS Vendor-Delta (Platzhalter Phase 1)
│   │   │   └── fastdds.rs       # Fast-DDS Vendor-Delta (Platzhalter Phase 1)
│   │   ├── compose.rs           # Base + N Deltas → effektive Grammar
│   │   └── validate.rs          # Ambiguity/Linksrekursion/Unused-Checks
│   ├── preprocessor/
│   │   ├── mod.rs               # #include/#define/#ifdef/#pragma
│   │   ├── expand.rs            # Makro-Expansion
│   │   └── source_map.rs        # Line/Column-Mapping zurueck auf Original-File
│   ├── lexer/
│   │   ├── mod.rs               # Generischer Token-Erzeuger
│   │   ├── token.rs             # TokenKind, Token, TokenStream
│   │   └── rules.rs             # Token-Regel-Extraktion aus Grammar
│   ├── engine/
│   │   ├── mod.rs               # Earley-Parse-Engine
│   │   ├── state.rs             # Earley-Items und State-Sets
│   │   └── recognize.rs         # Recognition-Phase
│   ├── cst/
│   │   ├── mod.rs               # Untypisierter CST
│   │   ├── node.rs              # CstNode { production_id, children, span }
│   │   └── walk.rs              # Tree-Traversal
│   ├── ast/
│   │   ├── mod.rs               # Typed AST: ModuleSet, Module, Decl, Type, ...
│   │   ├── types.rs             # ~60 Node-Typen
│   │   ├── builder.rs           # CST → typed AST
│   │   └── print.rs             # Debug-Pretty-Print
│   └── errors.rs                # ParseError, Span, Diagnostic
└── tests/
    ├── fixtures/
    │   ├── omg/                 # zerodds_dcps.idl, builtin_topic_data.idl
    │   ├── rti/                 # RTI-SDK-Samples (via fetch.sh)
    │   ├── cyclonedds/          # Cyclone-DDS-Samples
    │   └── fastdds/             # Fast-DDS-Samples (Phase 1)
    ├── fetch.sh                 # Vendor-Fixture-Downloader
    ├── parse_omg.rs             # OMG-Referenz-Tests
    ├── parse_rti.rs             # RTI-Delta-Tests
    ├── grammar_coverage.rs      # Coverage-Matrix-Test
    └── roundtrip.rs             # CST → text → CST Roundtrip
```

### 4.3 Public API (Vorschlag, verbindlich ab Phase 1)

```rust
/// Hauptentrypunkt.
pub fn parse(source: &str, config: &ParserConfig) -> Result<Ast, ParseError>;

/// Konfiguration fuer Versions- und Vendor-Varianten.
pub struct ParserConfig {
    pub version: IdlVersion,                    // default: V4_2
    pub compat: CompatMode,                     // default: Strict
    pub vendor_extensions: Vec<VendorExt>,      // default: leer
    pub preprocessor_defines: Vec<(String, String)>,
    pub include_paths: Vec<PathBuf>,
}

pub enum IdlVersion { V3_5, V4_0, V4_1, V4_2 }
pub enum CompatMode { Strict, Relaxed }
pub enum VendorExt { RtiConnext, CycloneDds, FastDds, OpenDds }

/// Typisierter AST.
pub struct Ast {
    pub modules: Vec<Module>,
    pub source_map: SourceMap,
}
```

## 5 Detailliertes Design

### 5.1 Grammar-Daten-Modell

```rust
pub struct Grammar {
    pub name: &'static str,                 // "IDL 4.2"
    pub version: IdlVersion,
    pub productions: &'static [Production],
    pub start_symbol: SymbolId,
    pub token_rules: &'static [TokenRule],
}

pub struct Production {
    pub id: ProductionId,
    pub name: &'static str,                 // "struct_def"
    pub spec_ref: SpecRef,                  // SpecRef { doc: IDL_4_2, section: "7.4.1.4.4.2" }
    pub alternatives: &'static [Alternative],
    pub ast_hint: Option<AstHint>,          // Optionale Builder-Metadaten
}

pub struct Alternative {
    pub symbols: &'static [Symbol],
    pub note: Option<&'static str>,         // Fuer Grammar-Reviews
}

pub enum Symbol {
    Terminal(TokenKind),                    // z.B. TokenKind::Keyword("struct")
    Nonterminal(ProductionId),              // Ref auf andere Production
    Repeat(RepeatKind, &'static [Symbol]),  // {x}*, {x}+, [x]
    Choice(&'static [&'static [Symbol]]),   // Inline-Alternative
}

pub enum RepeatKind { ZeroOrMore, OneOrMore, Optional }
```

**Warum `&'static` / `const`:**

- Keine Heap-Allokation fuer die Grammar selbst zur Laufzeit; Grammar
  liegt im Binary-Segment.
- `const`-evaluierbar → Compile-Zeit-Validierung in zukuenftigen
  Rust-Versionen moeglich.
- Diff-bar in Git: Grammar-Aenderungen sind Zeilen-Diffs in `.rs`-Dateien.

**Alternative erwogen, abgelehnt:**

- BNF-Textformat (`.bnf`-Files + Loader): erzeugt ein Bootstrap-Problem
  (Wir braeuchten einen BNF-Parser um die Grammar zu laden). Daten in Rust
  umgehen das elegant.
- Proc-Macro-DSL: verschoene Syntax, aber versteckt die Daten-Struktur.
  Review-Freundlichkeit leidet.

### 5.2 Earley-Engine

**Warum Earley statt PEG/LL(k)/GLR:**

| Kriterium | Earley | PEG | LL(k) | GLR |
|---|---|---|---|---|
| Handelt beliebige CFG | ✓ | ✓ (aber greedy) | — (k-Lookahead) | ✓ |
| Robust bei Grammar-Mutation (Delta-Composition) | ✓ | — (Ordering-Traps) | — | ✓ |
| Implementations-Komplexitaet | mittel | niedrig | niedrig | hoch |
| Lineare Zeit fuer LR-Grammatiken | ✓ | ✓ | ✓ | ✓ |
| Worst-Case-Komplexitaet | O(n³) | O(n) Packrat | O(n) | O(n³) |
| Fehler-Diagnose (Expected-Set) | ✓ (trivial) | mittel | ✓ | komplex |

IDL-Grammatiken sind nicht LL(1) (z.B. Struct vs. Forward-Decl braucht Lookahead),
haben teils ambige Regeln in Kombination mit Deltas (Vendor-Annotations koennen
syntaktisch mit OMG-Keywords kollidieren). Earley handelt das sauber.

Performance: IDL-Files ueblicherweise 100–10.000 LOC. Bei O(n²) effektiver
Komplexitaet (unambiguous Bereich) fuer 10k-LOC-Input entspricht das
~10⁸ Operationen — im Millisekundenbereich. Irrelevant.

**Implementierung:**

Klassischer Earley (Scan/Predict/Complete) mit State-Sets als `Vec<EarleyItem>`.
Keine Packrat-Memoisation in Phase 0 (kommt nur bei Performance-Bedarf).

```rust
pub struct Engine<'g> {
    grammar: &'g Grammar,
}

impl<'g> Engine<'g> {
    pub fn recognize(&self, tokens: &[Token]) -> Result<ParseForest, ParseError>;
    pub fn build_cst(&self, forest: ParseForest) -> Result<Cst, ParseError>;
}
```

### 5.3 Delta-Composition

**Konzept:** Base-Grammar (`IDL_42`) wird mit einer Sequenz von Deltas
kombiniert. Jedes Delta enthaelt **Add**-, **Replace**- und **Remove**-
Operationen auf Productions.

```rust
pub struct GrammarDelta {
    pub name: &'static str,
    pub applies_to: IdlVersion,
    pub operations: &'static [DeltaOp],
}

pub enum DeltaOp {
    AddProduction(Production),
    ReplaceProduction { id: ProductionId, with: Production },
    RemoveProduction(ProductionId),
    AddAlternative { production: ProductionId, alt: Alternative },
}

pub fn compose(base: &Grammar, deltas: &[&GrammarDelta]) -> Grammar;
```

**Beispiel RTI-Connext-Delta:**

```rust
// crates/idl/src/grammar/deltas/rti_connext.rs
pub const RTI_CONNEXT: GrammarDelta = GrammarDelta {
    name: "RTI Connext 7.x",
    applies_to: IdlVersion::V4_2,
    operations: &[
        DeltaOp::AddAlternative {
            production: PROD_ANNOTATION_APPL,
            alt: Alternative {
                symbols: &[
                    Symbol::Terminal(TokenKind::Punct("@")),
                    Symbol::Terminal(TokenKind::Ident("transfer_mode")),
                    Symbol::Terminal(TokenKind::Punct("(")),
                    Symbol::Terminal(TokenKind::Ident("_any_")),
                    Symbol::Terminal(TokenKind::Punct(")")),
                ],
                note: Some("RTI proprietary, see RTI Connext 7.x User Manual §5.3"),
            },
        },
        // ... weitere Vendor-Annotations
    ],
};
```

### 5.4 CST vs. typed AST

**Zweistufiger Aufbau:**

- **CST** (`cst::CstNode`): Baum aus `{production_id, children, span}`.
  Keine typisierte Semantik. Preserves whitespace/comments (fuer spaetere
  Formatter-Use-Cases).
- **Typed AST** (`ast::Ast`): Stark typisierte Enum-Baeume: `Module`,
  `Decl` (Struct/Enum/Union/Typedef/Interface/…), `Type` (Primitive/Named/
  Sequence/Array/Map/…), `Annotation`, `Literal`. Ca. 60 Node-Typen.

**Warum zweistufig:**

- Grammar bleibt reine Daten — keine Builder-Closures pro Production.
- AST-Schema kann unabhaengig von Grammar evolvieren (z.B. neue Lint-
  Analysen ueber AST, ohne Grammar zu beruehren).
- CST ist die richtige Datenstruktur fuer Source-Preserving Rewrites,
  Error-Recovery, Error-Reporting mit exakten Spans. Das ist der
  rust-analyzer/rowan-Ansatz.

**AST-Builder:** Rekursiver Traversal ueber CST, match auf `production_id`
bzw. `production_name`. Per-Production-Konstruktor-Funktion, benannt wie
die Production (`build_struct_def(cst_node) -> StructDef`). Parallel zur
Grammar strukturiert.

### 5.5 Error-Reporting

- **Span-tracking** vom Lexer bis in den AST: jedes AST-Node traegt
  `Span { start: SourceLoc, end: SourceLoc }`.
- **Diagnostic-Format** inspiriert von rustc: Level (Error/Warning),
  Message, Primary-Span, optional Secondary-Spans mit Hinweis-Texten.
- **Earley-Engine liefert Expected-Set** bei Parse-Failure („expected one
  of: `struct`, `module`, `typedef`, …") durch Inspektion der aktiven
  Earley-Items.
- **Preprocessor-Source-Map**: Positionen in expanded-source werden auf
  Original-Source (vor `#include`-Resolution) abgebildet.

### 5.6 Vendor-Fixture-Beschaffung

Test-Fixtures aus Open-Source-Repos werden via `tests/fixtures/fetch.sh`
geholt (analog zu `docs/standards/fetch.sh`). Apache-2.0-lizenzierte
Fixtures koennen ins Repo committed werden; Datei-Lizenz-Header prueft
`cargo-deny` nicht — wir pflegen eine `LICENSES.md` pro Fixture-Subdir.

Fuer proprietaere RTI-Connext-IDL-Beispiele aus der Community Edition:
Fixtures nur downloaden, nicht committen. `.gitignore` fuer `tests/
fixtures/rti/downloaded/`.

## 6 Scope und Deliverables (Spike-DoD)

Der Spike ist abgeschlossen, wenn:

1. `cargo test -p zerodds-idl` ist gruen.
2. `zerodds-idl::parse(source, &ParserConfig::default())` parst `zerodds_dcps.idl`
   aus der OMG-Spec (liegt im Cache unter `docs/standards/cache/omg/`) und
   emittiert einen vollstaendigen typisierten AST.
3. Mindestens **ein Vendor-Delta** (RTI _oder_ Cyclone) ist implementiert
   und laesst mindestens eine echte Vendor-IDL-Datei erfolgreich parsen,
   die ohne Delta fehlschlagen wuerde.
4. `tools/idlc --parse-only <file.idl>` ruft `zerodds-idl::parse` und druckt
   den AST via Debug-Format.
5. **Coverage-Tool** listet auf, welche OMG-IDL-4.2-Grammar-Productions
   durch welche Test-Fixtures abgedeckt sind. Coverage muss ≥80% auf
   `zerodds_dcps.idl`-Pfad betragen.
6. **Migration-Guide-Skelette** unter `documentation/user-guide/migrations/`
   existieren (`from-rti.md`, `from-cyclone.md`, `from-fastdds.md`) mit
   Platzhalter-Struktur; Inhalt wird schrittweise befuellt.
7. **Grammar-Validation** laeuft als Test: keine Ambiguitaet, keine
   Linksrekursion, keine unbenutzten Nonterminals in `IDL_42`.
8. Public-API ist dokumentiert (`cargo doc -p zerodds-idl` produziert gruene Docs).

## 7 Test-Strategie

| Kategorie | Tools | Wann | Scope |
|---|---|---|---|
| Unit | `cargo test -p zerodds-idl` | Bei jedem Commit | Engine-Internals, Grammar-Validation, Delta-Composition |
| Grammar-Coverage | Custom test | Nightly | Welche Productions werden durch welche Fixtures abgedeckt |
| Fixture-Tests | `cargo test --test parse_omg`, `parse_rti` | Bei jedem Commit | Echte IDL-Files |
| Roundtrip | Custom | Bei jedem Commit | Source → CST → Rendered-Source == Source (Whitespace-tolerant) |
| Property-Based | `proptest` | Optional Phase 1 | Random-Grammar-Input, Engine-Invarianten |
| Fuzz | `cargo-fuzz` | Nightly (ab Phase 1) | Preprocessor + Lexer |

**Test-Fixture-Quellen (Lizenz-sauber):**

| Quelle | Lizenz | Abdeckung |
|---|---|---|
| OMG DDS 1.4 Spec Annex | Spec-lizenziert | DCPS-Referenz-IDL |
| Cyclone DDS GitHub | EPL-2.0 / EDL-1.0 | Cyclone-Dialekt-Tests |
| Fast DDS GitHub | Apache-2.0 | Fast-DDS-Dialekt-Tests |
| OpenDDS GitHub | Nicht-virale License | OpenDDS-Dialekt-Tests |
| RTI Connext Community Edition | Proprietaer, kostenlos | RTI-Dialekt-Tests (nur lokal, nicht committen) |

## 8 Alternativen-Analyse

### 8.1 Handgeschriebener Recursive-Descent-Parser

**Pro:** Einfachste Implementierung, keine Engine-Infrastruktur, schneller
initialer Durchlauf (~2–3 Wochen fuer IDL-4.2-Subset).

**Kontra:** Jede Vendor-Delta erfordert Code-Aenderungen am Parser. Das
skaliert nicht auf unser Ziel von „Migration aus 4+ Vendor-Stacks ohne
Engine-Refactoring". Spec-Update-Diff gegen handgeschriebenen Code ist
aufwendiger als Grammar-Daten-Diff. Traceability-Anker (`spec_ref`) muss
manuell in Kommentaren gepflegt werden — driftet.

**Verworfen.** Sales-Argument aus §2.1 uebersteigt den Initial-Aufwand-
Unterschied.

### 8.2 Externes Parser-Generator-Tool (`pest`, `lalrpop`, `tree-sitter`)

**Pro:** Etablierte Projekte, deklarative Grammar-Syntax.

**Kontra:**
- `pest`, `lalrpop`: Build-Zeit-Codegen. Grammar-Delta-Composition zur
  Laufzeit waere nicht moeglich — wir muessten pro Vendor-Delta separat
  kompilieren.
- `tree-sitter`: C-basierte Laufzeit-Bibliothek, externes Dep, nicht
  Pure-Rust, zusaetzliche Build-Komplexitaet (C-Compiler nötig).
- Alle: Fuegen Dep-Review-Arbeit hinzu (siehe `02_architecture.md §4.3`
  Dep-Politik fuer Safe-Klassifikation).

**Verworfen.** Zusaetzlich: das „Parser-Parser"-Paradigma ist hier
Produkt-Feature, nicht nur Implementierungs-Detail.

### 8.3 PEG-Engine statt Earley

**Pro:** Einfacher zu implementieren, linearzeit mit Packrat.

**Kontra:** PEG ist **greedy** — Alternativen-Ordering entscheidet
Parse-Ergebnisse. Delta-Composition wuerde die Reihenfolge von
Alternativen aenderbar machen, was in PEG zu schwer debugbaren
Parse-Unterschieden fuehren kann. Vendor-Delta-Zusaetze koennten
unvorhersehbar den Base-Grammar-Parse ueberschreiben.

**Verworfen.** Earley's Gleichberechtigung aller Alternativen macht
Delta-Composition sauber.

## 9 Risiken

| ID | Risiko | Wahrscheinlichkeit | Impact | Mitigation |
|---|---|---|---|---|
| R-1 | BNF-Transkription aus OMG-PDF fehlerhaft | Mittel | Mittel | Lead-Engineer-Review pro transkribierter Section; Grammar-Coverage-Test als Netz |
| R-2 | Earley-Engine hat subtile Bugs (Edge-Cases bei epsilon-Produktionen o.ae.) | Mittel | Hoch | Start mit etablierten Referenz-Implementierungen (Papier Aycock/Horspool 2002); ausgiebige Unit-Tests |
| R-3 | Performance ungenuegend auf grossen IDL-Files | Niedrig | Niedrig | IDL-Files sind klein (<10k LOC); Performance-Probleme waeren Algorithmus-Bug, kein Design-Problem |
| R-4 | Vendor-Delta zeigt, dass unsere Delta-Komposition Architektur nicht traegt | Niedrig | Hoch | Spike-DoD verlangt genau diesen Nachweis. Wenn Delta nicht funktioniert, ist das ein Invalidation-Result — Architektur wird neu bewertet |
| R-5 | CST/AST-Trennung erzeugt zu viel Boilerplate fuer ~60 Node-Typen | Mittel | Mittel | Code-Gen fuer AST-Builder (Macro oder externes Script) als Fallback; Claude-Augmentation stark |
| R-6 | Preprocessor-Semantik ist subtiler als geplant (Macro-Expansion mit `##`, `#`, varargs) | Mittel | Mittel | OMG-IDL-4.2 unterstuetzt eingeschraenkten Preprocessor — wir implementieren genau diesen Umfang, keine C-Preprocessor-Parity |
| R-7 | Scope-Creep durch Vendor-Delta-Vielfalt | Hoch | Mittel | Spike limitiert auf **einen** Vendor-Delta; weitere als Phase-1-Arbeit |

## 10 Offene Fragen

- **Q-1:** Sollen wir bereits im Spike einen Ambiguity-Resolution-Mechanismus
  (z.B. Production-Priorities) einfuehren, oder erst wenn erste Ambiguitaet
  in realer Grammar auftritt? **Empfehlung:** erstmal nicht, Grammar
  bewusst eindeutig halten.
- **Q-2:** `ParserConfig.vendor_extensions: Vec<VendorExt>` — mehrere
  Vendor-Deltas gleichzeitig erlauben? Ein File koennte theoretisch
  RTI+Cyclone-Mix sein, ist aber unrealistisch. **Empfehlung:**
  mehrere erlauben, aber Reihenfolge-unabhaengige Composition nicht
  garantieren (erster Delta hat semantischen Vorrang bei Konflikt).
- **Q-3:** CST-Arena-Allokation (Index-basiert) oder `Box`-Tree?
  **Empfehlung:** `Box`-Tree im Spike (einfach), Arena-Migration als
  Optimierung nach Profiling.
- **Q-4:** `SourceMap` als eigenen Sub-Crate? **Empfehlung:** nein,
  crate-intern belassen. Falls `zerodds-xml` spaeter auch Source-Maps
  braucht, kann refactored werden.

## 11 Referenzen

- OMG IDL 4.2 Spec: `docs/standards/cache/omg/idl-4.2.pdf` (Section §7.2
  Lexical, §7.3 Preprocessing, §7.4 Syntactic)
- OMG DDS 1.4 Spec Annex (Referenz-IDL-Files): `docs/standards/cache/omg/zerodds-dcps-1.4.pdf`
- Aycock & Horspool, „Practical Earley Parsing", The Computer Journal 45(6), 2002
- rust-analyzer `rowan` Crate (CST-Modell-Inspiration)
- `docs/architecture/02_architecture.md` §3.3, §4.4 (Crate-Klassifikation)
- `docs/architecture/04_safety_by_architecture.md` §4 (Traceability-Anker)
- `docs/architecture/07_risks_and_strategy.md` §3.2 (Migration-Positionierung)
- `.planning/wp-0.3-idl-parser/PLAN.md` (Wochen-Breakdown)

## 12 Revisions-Log

| Datum | Revision | Autor | Notiz |
|---|---|---|---|
| 2026-04-17 | Draft v0.1 | Core Team | Initiale Fassung |
