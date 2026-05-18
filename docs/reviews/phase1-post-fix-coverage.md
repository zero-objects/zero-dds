# Phase-1 Post-Fix Coverage Audit

**Datum:** 2026-04-20 nach realem P1-Fix inkl. P1-4 (Duration/DurabilityKind-
Konsolidierung) und P1-6 (`CacheChange.payload: Arc<[u8]>`).
**Quelle:** `cargo llvm-cov --workspace --summary-only`.

## Workspace TOTAL

| Metric    | Pre-Fix (phase1-coverage-audit) | Post-Fix (heute) | Delta     |
|-----------|---------------------------------|------------------|-----------|
| Regions   | 81.67 %                         | **81.62 %**      | -0.05 pp  |
| Functions | 92.34 %                         | **92.34 %**      |  0.00 pp  |
| Lines     | 92.17 %                         | **92.16 %**      | -0.01 pp  |

Praktisch unveraendert. Die Arc-Payload-Umstellung war chirurgisch — ein
neuer Konstruktor + eine Generic-Signatur — ohne messbaren Coverage-Impact.
Kein neuer Region-Swell durch Monomorphisierung, weil `decode_*_samples`
zurzeit nur mit **einer** Monomorphisierung (`Arc<[u8]>`) aufgerufen wird.

## P1-Fix Hot-Spots + aktuelle Coverage

| Datei                                  | R %    | L %    | Relevanz                                       |
|----------------------------------------|--------|--------|------------------------------------------------|
| `crates/rtps/src/history_cache.rs`     | 95.29  | 97.67  | **P1-6**: `CacheChange.payload: Arc<[u8]>`, neuer `alive_arc()` |
| `crates/discovery/src/sedp/reader.rs`  | 68.66  | 77.65  | **P1-6**: `decode_*_samples<B: AsRef<[u8]>>`   |
| `crates/rtps/src/reliable_writer.rs`   | 81.33  | 91.56  | **P1-3**: `debug_assert_eq` Guards; einziger Call-Site fuer `alive_arc` |
| `crates/rtps/src/error.rs`             | 91.67  | 87.30  | **P1-5**: `#[non_exhaustive]` auf `WireError`  |
| `crates/types/src/error.rs`            | 78.95  | 100.00 | **P1-5**: `#[non_exhaustive]` auf `TypeCodecError` |
| `crates/qos/src/pid.rs`                | —      | 100.00 | **P1-5**: non_exhaustive (Match-Arms bleiben vollstaendig) |
| `crates/rtps/src/parameter_list.rs`    | 93.15  | 96.57  | **P1-1**: `MAX_PARAMETERS=4096` DoS-Cap       |
| `crates/types/src/type_lookup.rs`      | 84.62  | 94.68  | **P1-2**: `safe_capacity`-Pfad                |
| `crates/qos/src/duration.rs`           | 95.56  | 100.00 | **P1-4** (doc-only, aber Duration-Mod konsolidiert) |
| `crates/qos/src/policies/durability.rs`| 92.86  | 96.97  | **P1-4** (doc-only, DurabilityKind)            |

### Beobachtungen zu den Fixes

- **`alive_arc()` hat keinen direkten Unit-Test**, wird aber indirekt ueber
  `alive()` (Tests in `history_cache::tests`) und einmal in `reliable_writer`
  erreicht. Die 4 uncovered Regions in `history_cache.rs` liegen in
  `remove_up_to`-Edge und `insert`-Race-Pfaden, **nicht** im neuen Konstruktor.
- **`decode_*_samples` ist generisch** ueber `B: AsRef<[u8]>`, wird aktuell
  aber **nur als `Arc<[u8]>`-Monomorphisierung** aufgerufen. Ein `Vec<u8>`-
  Call-Site existiert nicht — daraus entsteht kein dead code, weil `Arc::from(Vec)`
  im alten Pfad vorgelagert ist, aber fuer eine zweite Monomorphisierung
  fehlt ein expliziter Test.
- **`non_exhaustive`-Enums** haben alle exhaustive Match-Arms ohne `_ =>`
  Fallback (`WireError::fmt`, `TypeCodecError::fmt`, `CacheError`,
  `FragmentAssemblerError`, `EndpointMatchError`). Es gibt also keine neu
  toten Default-Arms — der Attribute-Fix ist reine API-Firewall.

## Top 3 Nachfolge-Tests (realistisch +1-2 pp TOTAL)

1. **`decode_*_samples` Vec-Monomorphisierung** + Arc-Duplikat
   (`sedp/reader.rs`): 2 Unit-Tests die je 1 Iterator mit `Vec<u8>`- und
   `Arc<[u8]>`-Payloads durch beide Decoder schicken → deckt Generic-
   Instantiation **und** Arc-Zero-Copy-Pfad ab. Schaetzung: **+15 R, +20 L**.
2. **`CacheChange::alive_arc` Direkt-Test** + Payload-Identity-Assertion
   (`Arc::ptr_eq(&orig, &cc.payload)`): dokumentiert Zero-Copy-Garantie und
   deckt den Konstruktor-Pfad ohne Umweg ueber `alive(Vec)`. Schaetzung:
   **+3 R, +4 L** (klein, aber semantisch wichtig).
3. **`subscription_data.rs` Roundtrip-Suite** (52.88 % R): gleiche Lücken
   wie bei `publication_data` — BE-Encoding, `UnsupportedEncapsulation`,
   PID_TYPE_NAME-Missing, DATA_REPRESENTATION non-empty. Nicht durch
   P1-Fixes verursacht, aber groesster relativer Gap im RTPS-Layer.
   Schaetzung: **+40 R, +60 L**.

## Verdict

Die P1-Fixes haben **keine neuen ungedeckten Pfade** eingefuehrt.
Workspace-TOTAL ist innerhalb der Mess-Schwankung (+/-0.05 pp). Der
einzige sinnvolle Nachzieh-Push ist Punkt 1 — zweite Monomorphisierung
fuer `decode_*_samples` — als **Assertion** der Arc-Zero-Copy-Umstellung,
nicht als Coverage-Trick.
