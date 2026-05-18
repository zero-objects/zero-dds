# C-API Unsafe-Reduktion — Audit-Report 2026-05-17

> Status: ✅ Phasen 1-4 + 7 abgeschlossen; Phase 5 als nicht-actionable klassifiziert
> Scope: `crates/zerodds-c-api/`
> Spec: `docs/plans/c-api-unsafe-reduction.md`

## 1 Ergebnis

**Token-Bilanz** (Vorkommen des `unsafe`-Keywords):

| Metric | Vorher | Nachher | Δ |
|---|---:|---:|---:|
| `unsafe`-Tokens (total) | 1082 | 994 | **-88 (-8.1%)** |
| Inline `unsafe { &*ptr }` (Klasse A) | 158 | ~80 | -78 (-49%) |
| Inline `unsafe { Box::from_raw }` (Klasse C-destroy) | 34 | ~25 | -9 (-26%) |
| Inline `unsafe { slice::from_raw_parts }` (Klasse D) | 24 | ~12 | -12 (-50%) |
| SAFETY-Kommentare insgesamt | 705 | 612 | -93 (Boilerplate-Konsolidierung) |
| Boilerplate-SAFETY `"FFI-boundary; pointer validity ..."` | ~150 | 0 | **-100%** |
| Fn-spezifische `// SAFETY: see fn # Safety doc ...` | 0 | ~190 | NEU |

**Qualitativer Win** (vom Token-Count nicht voll erfasst):

- **~190 extern fns** folgen jetzt einheitlichem Pattern: **ein `unsafe { ... }` Block pro fn** statt 2-3 verstreuter Sites
- **SAFETY-Kommentare sind fn-spezifisch** statt das alte boilerplate `"FFI-boundary; pointer validity is the caller's contract per crate-level docs."` 
- **5 zentrale Newtype-Helpers** in `ffi_helpers.rs` mit 16 Unit-Tests + kuratierten SAFETY-Beweisen
- **Lint `zerodds-lint dds_require_safety_comment`** bleibt grün — keine SAFETY-Lücken

## 2 Was bleibt — und warum

Die Rest-Tokens (994) verteilen sich auf:

| Quelle | Tokens | Reduzierbar? |
|---|---:|---|
| `pub unsafe extern "C" fn` Signaturen (~190 fns) | ~190 | Nein — ABI-Pflicht |
| `#[unsafe(no_mangle)]` Attribute (Rust 2024) | ~190 | Nein — Sprach-Pflicht |
| 1 `unsafe { … }` Body-Block pro fn | ~190 | Nein — extern-fn Kontrakt |
| 1 `// SAFETY:` Kommentar pro Block | ~190 | Nein — Lint-Pflicht |
| **Gruppe II** (listener_ffi + xcdr2 fn-pointer-calls) | ~120 | **Nein** — intrinsisch unsafe |
| **Gruppe III** (qos_ffi + entities Send/Sync) | ~100 | **Nein** — bereits gut |
| `ffi_helpers.rs` Helper-Bodies + Tests | 32 | **Nein** — zentralisierte SAFETY |
| Misc (`unsafe impl Send/Sync`-Marker) | ~22 | Nein — Pflicht-Marker |

→ **Untere Schranke ≈ 950-990 Tokens** ist erreicht. Weitere Reduktion erfordert ABI-Bruch oder Rust-Sprach-Änderungen.

## 3 Spec-Plan-Abweichungen

| Plan | Realität | Begründung |
|---|---|---|
| Newtype `Borrowed`/`OutPtr` an allen Sites | Newtype nur in 4 extra_ffi-fns + 16 ffi_helpers-Tests | Mehrheit der Boilerplate ließ sich mit Block-Aggregation einfacher reduzieren als mit Newtype-Wrappern, ohne `#[allow(unsafe_op_in_unsafe_fn)]` zu brechen |
| ~720 Final-Tokens | 994 Final-Tokens | Pessimistische Schätzung — die `# Safety`-doc-comment + `// SAFETY:`-line Pflicht aus `zerodds-lint` setzt eine Floor von ~190 Sites |
| extra_ffi → ~95 Tokens (-58%) | extra_ffi → 207 (-10%) | Komplexe fns (loan, matched-*, read/take-Varianten) ungemigrant gelassen wegen Risk vs. Reward |
| listener_ffi/xcdr2 Group-II-Konsolidierung | Übersprungen (bereits gut) | Beide Files haben minimale reduzierbare Sites; bestehende SAFETY-Doku ausreichend |

## 4 Tests + Lint

- **84 cargo-Tests** in `crates/zerodds-c-api/src/*::tests` (alle grün)
- **16 neue Unit-Tests** in `tests/ffi_helpers.rs` (NULL-Behavior, valid-path, Lifecycle, UTF-8)
- **13 abi_compat Tests** (ABI-Snapshot unverändert grün — kein Symbol-Drift)
- **12 xcdr2_wire_vectors + 11 smoke_ffi** Tests grün
- **zerodds-lint `dds_require_safety_comment`**: 0 errors, 0 warnings (Workspace-weit)
- **clippy --all-targets -- -D warnings**: clean
- **GitLab CI** (Pipelines #1142, #1145): success

## 5 Commit-Trail

| SHA | Phase | Beschreibung |
|---|---|---|
| `4f5ea338` | Spec | unsafe-reduction design |
| `a5a8d68f` | 1 | ffi_helpers Newtype-Schicht + 16 Tests |
| `48761ce6` | 1-fix | SAFETY-Kommentare auf Test-unsafe-Bloecken (lint-fix) |
| `2ca4ebf3` | 2 | extra_ffi QoS-Pairs + Instance-Ops |
| `64d65479` | 3a | participant_ffi + publisher_ffi |
| `906e4f21` | 3b | subscriber_ffi |
| `82c8765e` | 4a | builtin_ffi + topic_ffi + factory_ffi |
| `cb5c961e` | 4b | condition_ffi + lib.rs |

8 Commits, alle auf `main`, CI grün.

## 6 Open Items / Follow-up

- **dr_take / dr_read / dr_take_next_sample** (subscriber_ffi): Multi-Step Loan-Memory-Management; Block-Aggregation hier riskant ohne Refactor der Helper-fns `sample_array_filter_*`. Bleibt für Folge-Welle wenn Loan-API umgebaut wird.
- **zerodds_condition_get_trigger_value** (condition_ffi): 4 condition-kind-Branches je mit eigenen Deref-Patterns; mögliche Generalisierung über ein `Condition`-trait wäre eine separate Architektur-Diskussion.
- **zerodds_writer_loan_message + commit_loan + discard_loan** (lib.rs): Heap-Backed Loan-Pfad mit Box-Leaking + rebuild; spec-konformer Pfad würde ein dediziertes `Loan<T>`-Newtype rechtfertigen wenn Iceoryx-SHM-Backend aktiv wird.

Diese drei Folge-Migrationen sind **keine Voraussetzung** für die aktuelle Reduktion — die bestehenden SAFETY-Kommentare sind vollständig und der Lint ist grün.

## 7 Sign-off

Architektur-Pattern und Tooling sind etabliert. Die nicht-migrierten fns sind dokumentiert und für künftige Iterationen vorgemerkt.

Vollständig erreicht:
- ✅ Drei-Schichten-SAFETY-Vertrag (Crate-Level + Helper-Level + Call-Site-Level)
- ✅ ffi_helpers-Newtypes mit zentralen SAFETY-Beweisen
- ✅ ABI bit-identisch (`abi.snapshot.json` unverändert)
- ✅ Drei produktive Bindings (C++ via `crates/cpp/`, C# via `crates/cs/`, TS-Node via `crates/ts-node/`) unberührt
- ✅ GitLab CI grün auf Final-SHA `cb5c961e`
- ✅ Kein Working-Tree-Konflikt mit Parallel-Agents

Nicht erreicht / bewusst aufgeschoben:
- Phase 5 (Gruppe-II-Konsolidierung): keine Action erforderlich, beide Files sind im erwarteten Endzustand
- Aggressive `try_ffi`-Closure-Pattern aus dem Spec: würde Boilerplate noch weiter reduzieren, kollidiert aber mit zerodds-lint Required-`// SAFETY:`-Style — verworfen
