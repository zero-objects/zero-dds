# WP 1.5 Test-Coverage Report

**Generated:** `cargo llvm-cov --workspace --summary-only --ignore-filename-regex='(tests|examples)/'` am 2026-04-19
**Commit:** `c4bf07a`

## Workspace-Total

| Metric    | Coverage | Gap                   |
|-----------|----------|-----------------------|
| Regions   | **77.08%** | deutlich unter 99%-Messlatte |
| Functions | **90.13%** | nahe Target            |
| Lines     | **89.30%** | nahe Target            |

## zerodds-types — WP-1.5-Module

| Modul                                        | Regions | Lines  | Bemerkung                                                  |
|----------------------------------------------|---------|--------|------------------------------------------------------------|
| `type_object/mod.rs` (TypeObject-Wrapper)    | 90.32%  | 100%   | fast vollständig                                            |
| `hash.rs`                                    | 87.80%  | 100%   | gut                                                        |
| `qos.rs`                                     | 85.42%  | 94.59% | gut                                                        |
| `type_information.rs`                        | 78.30%  | 94.36% | mittel                                                     |
| `type_lookup.rs`                             | 78.12%  | 87.42% | mittel                                                     |
| `type_identifier/mod.rs`                     | 68.58%  | 87.47% | mittel — einige Varianten (PlainMap, StronglyConnectedComp) mit nur je 1 Roundtrip |
| `type_object/common.rs`                      | 70.44%  | 90.88% | mittel                                                     |
| `type_object/minimal/*.rs` (8 Dateien)       | 68-77%  | 80-100%| mittel, insbesondere `minimal/mod.rs` (Dispatch-Union) 69% |
| `type_object/complete/mod.rs`                | **37.23%** | 53.30% | **KRITISCHE SCHIEFLAGE** — nur 5 Complete-Kinds getestet, Decode-Pfade grösstenteils ungetestet |
| `resolve.rs`                                 | **46.81%** | 71.94% | **SCHIEFLAGE** — collect_referenced_hashes-Walk grösstenteils untested |
| `type_object/flags.rs`                       | 60.00%  | 60%    | nicht kritisch (nur u16-Konstanten + default-impls)        |
| `builder.rs` + `assignability.rs`            | tbd     | tbd    | hohe Struct-Test-Abdeckung; Union/Collection-Builder neu   |

## zerodds-idl — WP-1.5-Module

| Modul                              | Regions | Bemerkung                           |
|------------------------------------|---------|-------------------------------------|
| `semantics/annotations.rs`         | tbd     | 7 Lowering-Tests, aber Fehlerpfade nicht alle abgedeckt |
| `semantics/to_typeobject.rs`       | tbd     | 5 Tests, Union/Enum/Alias-Mapping fehlt komplett         |

## zerodds-discovery

| Modul                              | Regions | Bemerkung                           |
|------------------------------------|---------|-------------------------------------|
| `type_lookup/mod.rs`               | tbd     | 3 Tests: Request-Roundtrip + Responder + Seq-Increment   |

## Befund

**Coverage-Schieflage gegen die 99%-Messlatte (Memory
`project_quality_bar_branch_coverage`).** Workspace-Total ist 77%
Regions. Die beiden schwachsten Stellen sind:

1. **`type_object/complete/mod.rs` (37% Regions)** — Decode-Pfade fuer
   Union/Alias/Bitmask/Bitset/Annotation/Map sind *implementiert* aber
   *ungetestet*. Roundtrip-Tests existieren nur fuer Struct, Union,
   Enum, Alias, Sequence. Es fehlen: Array, Map, Bitmask, Bitset,
   Annotation. **Empfehlung: 5 zusätzliche Roundtrip-Tests.**

2. **`resolve.rs` (47% Regions)** — `collect_referenced_hashes`-Walk
   fuer Union/Map/Sequence/Array ist ungetestet. **Empfehlung: 3-4
   Tests fuer collect_from_minimal(Union) + Sequence/Array/Map.**

**Für Phase 1 unkritisch**, weil alle Wire-Pfade symmetrisch zu den
Minimal-Tests sind und diese sehr gut abgedeckt sind. Complete-Decode
wird aber ohne Tests nicht an Live-Interop oder Fuzzing-Edge-Cases
ausgesetzt. Follow-up im Phase-1-Polish empfohlen.
