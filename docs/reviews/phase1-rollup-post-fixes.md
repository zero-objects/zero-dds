# Phase-1 Post-Fix Reevaluation

**Datum:** 2026-04-20 nach P1-Fix-Session.

## Fixes (7 Items)

| # | Status | Commit | Wirkung |
|---|--------|--------|---------|
| **P1-1** | ✅ done | 39cea45 | `ParameterList::MAX_PARAMETERS=4096` — SPDP/SEDP-DoS geschlossen |
| **P1-2** | ✅ done | 39cea45 | `type_lookup` nutzt `safe_capacity` — OOM-DoS via getTypes-Reply |
| **P1-3** | ✅ done | 39cea45 | ReliableWriter `debug_assert_eq` gegen Silent State-Drift |
| **P1-4** | 🟡 doc-only | b69b911 | Duration/DurabilityKind-Duplikation ehrlich dokumentiert, echte Konsolidierung → Phase 2 |
| **P1-5** | ✅ done | 39cea45 | 8 Error-Enums `#[non_exhaustive]` — Breaking-Change-Firewall |
| **P1-6** | 🟡 defered | 72abd06 | Arc&lt;[u8]&gt;-Payloads als eigener WP 2.0a-Spike in [phase2-spike-arc-payloads.md](phase2-spike-arc-payloads.md) |
| **P1-7** | ✅ done | 66ebae2 | recursion-depth-Marker in assignability klargestellt; 23 idl-Marker bleiben 64 (korrekt konservativ) |

## Reevaluation

**Tests:** workspace gruen, clippy clean, zerodds-lint clean.
**Coverage:** 81.66% R / 92.16% L — praktisch unveraendert (P1-Fixes
fuegten Doku + kleine Guards hinzu, kein neuer Code).
**Status:** Critical + Highs aus `phase1-rollup.md` Merge-Gate alle
adressiert. Phase-1-Abschluss ist ready.

## Offen fuer Phase-2-Start

- **WP 2.0a Zero-Copy-Payload-Spike** (1-1.5 PT) — P1-6 implementieren
  + Benchmark.
- **P2-Hygiene** aus `phase1-rollup.md`: O(n·m)-Assignability,
  SEDP-Cache-Insert-Linearitaet, SHM/TCP Silent-Lock-Failure, padding-
  reader-Striktheit, Platzhalter-Crates `publish=false`.

## Merge-Gate Phase-1-Close

✅ P1-Fixes committed + gepusht
✅ Pipeline gruen
✅ Keine Regression in Tests
✅ zerodds-lint clean
✅ Phase-2-Scope fuer offene Items dokumentiert

**Verdict: Phase-1 abgeschlossen. `gsd:complete-milestone` kann laufen.**
