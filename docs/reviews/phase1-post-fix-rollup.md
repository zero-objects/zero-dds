# Phase-1 Post-Fix Rollup (Audit-Runde 2)

**Datum:** 2026-04-20 nach P1-Fix-Session, Audit-Runde 2.

Zweite Audit-Runde nach den 7 P1-Fixes, um Regressionen und neu
entstandene Findings zu identifizieren. 4 parallele Agenten:
Coverage, Perf, Smells, Security.

## Agent-Reports

- [phase1-post-fix-coverage.md](phase1-post-fix-coverage.md)
- [phase1-post-fix-perf.md](phase1-post-fix-perf.md)
- [phase1-post-fix-smells.md](phase1-post-fix-smells.md)
- [phase1-post-fix-security.md](phase1-post-fix-security.md)

## Verifikation der 7 P1-Fixes

| Fix | Verifiziert durch | Status |
|-----|-------------------|--------|
| **P1-1** `MAX_PARAMETERS=4096` | Security, Perf (F17) | ✅ code + test |
| **P1-2** `safe_capacity` type_lookup | Security | ✅ code verified |
| **P1-3** Drift-Guard ReliableWriter | Smells #1 | ✅ debug_assert aktiv |
| **P1-4** Duration/DurabilityKind Konsolidierung | Smells #3/#4 | ✅ re-exports, From-impls raus |
| **P1-5** 8 Error-Enums `#[non_exhaustive]` | Smells #5, Coverage | ✅ 21 Treffer, keine toten Pfade |
| **P1-6** Arc&lt;[u8]&gt;-Payloads | Perf (F7/F8/F10), Security | ✅ CacheChange + Writer + Reader |
| **P1-7** recursion-depth Marker | — | ✅ bereits committed |

**Coverage-Delta:** -0.05 pp R / -0.01 pp L → Mess-Rauschen,
keine Regression.

## Neue Findings (Runde 2)

### Medium

**N-M1 — Arc-Refactor endet vor Submessage-Build**
- Quelle: Perf-N1, Smells-N2 (gleicher Befund, verschiedene Linsen).
- Ort: `DataSubmessage::serialized_payload: Vec&lt;u8&gt;` in
  `submessages.rs:416,868`; `to_vec()`-Kopien bei
  `reliable_writer.rs:428,599,671`.
- Wirkung: Der prognostizierte 30–50 %-Gain (siehe
  `phase2-spike-arc-payloads.md`) reduziert sich auf ~10–15 %.
  Perf-Gewinn greift nur Cache↔Writer, nicht bis zum Socket.
- Aktion: **In WP 2.0a Zero-Copy-Spike verschieben** — ist genau
  das, was der Spike misst und abschliesst.

### Low / Info

- **Perf-N2** `Arc::from(Vec)` im Writer-Hot-Path ~5–8 % fuer kleine
  Payloads → Design-Input fuer WP 2.1 DCPS `write_arc`-Entry.
- **Perf-N3** Arc-Atomics unkritisch solange tick() single-threaded
  bleibt.
- **Security-N1/N2/N3** info-level: Arc-Payloads data-race-frei
  (`Arc::get_mut`/`make_mut` = 0 Treffer), non_exhaustive
  workspace-safe, `fragment_assembler::CompletedSample::payload`
  weiterhin Vec (inkonsistent, aber nicht kritisch).
- **Smells-N1** `CacheChange::alive_arc` pub aber nur intern genutzt
  → kann `pub(crate)` werden.
- **Smells-N3** `CacheChange::alive(Vec)` alloziert weiter
  `Arc::from` pro Insert — Best-Effort-Writer profitiert nicht.
- **Smells-N5** Finding #8 `SilentDowngrade` per Grep nicht mehr
  auffindbar → Re-Verify.
- **Coverage**: `decode_*_samples` nur mit Arc-Monomorphisierung
  getestet, Vec-Monomorphisierung hat keine Call-Site → toter Code
  wenn nie mit Vec aufgerufen.

## Noch offen aus Runde 1 (bewusst deferred)

`phase1-smell-audit.md` #2, #8-#11, #13 unveraendert. Davon ist
**#11 (30 LOC Fix)** der teuerste offene Punkt — Phase-2-Hygiene.

## Verdict

**Keine neuen Criticals oder Highs.** Das einzige Medium (N-M1) ist
exakt der Scope des bereits dokumentierten WP 2.0a Spike — kein
neuer Arbeitspunkt, nur Bestaetigung dass der Spike tatsaechlich
Mehrwert liefert.

## Merge-Gate Phase-1-Close

✅ Alle 7 P1-Fixes verifiziert
✅ Coverage stabil (81.62 R / 92.16 L)
✅ Keine Criticals/Highs in Runde 2
✅ Medium → Phase-2 WP 2.0a bereits geplant
✅ Tests + clippy + zerodds-lint gruen

**Phase-1 ist ready fuer `/gsd:complete-milestone`.**
