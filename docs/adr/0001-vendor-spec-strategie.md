# 0001 — Vendor-Spec-Strategie für Lücken im OMG-Ökosystem

- **Status:** accepted
- **Datum:** 2026-05-04
- **Autoren:** @sandra
- **Kontext:** docs/specs/, docs/spec-coverage/

## Kontext

Bei der Audit-Vervollständigung (Phase-2 Spec-Coverage) sind zwei
Themen aufgekommen, die **keine OMG-Spec haben**, aber für ein
modernes DDS-Stack relevant sind:

- **async-DDS-API** — Tokio/async-std haben sich seit ~2019 als
  Standard-Pattern durchgesetzt. Existing DDS-Implementations (RTI,
  Cyclone, Fast-DDS) bieten C++-`std::future`-Wrappers, aber keine
  Spec-konforme `async fn`-API. dust-dds hat einen Tokio-only-Pfad.
- **Zero-Copy / FlatData** — RTI hat "FlatData" als Vendor-Erweiterung,
  Eclipse Iceoryx hat einen eigenen Standard, ROS-2 hat REP-2007-
  Loaning. Kein OMG-Konsens.

Wir brauchen eine **konsistente Strategie**, wie wir solche Lücken
schließen, ohne (a) die OMG-Compliance zu kompromittieren und (b) den
Eindruck zu erwecken, wir würden Standards umgehen.

## Entscheidung

**Wir definieren eigene Vendor-Specs unter `docs/specs/zerodds-<thema>-N.M.md`.**

Diese Specs:
- bekommen die gleiche Spec-Coverage-Behandlung wie OMG-Specs
  (`docs/spec-coverage/zerodds-<thema>-N.M.md` mit Items + Repo +
  Tests + Status, plus `.open.md`).
- haben eine eigene `## Decisions`-Sektion (D-1..D-N), die im
  PROCESS.md-Stil dokumentiert ist.
- starten als `Draft`, werden zu `Final` erst wenn alle Items done +
  Tooling fertig + funktionsnachweis erbracht ist.
- definieren eigene Wire-PIDs nur im Vendor-Bereich (>= 0x8000), mit
  `VENDOR_SPECIFIC_BIT` gesetzt und ohne `MUST_UNDERSTAND`.

## Alternativen

1. **Nur OMG-Specs umsetzen, Lücken offen lassen** — verlieren async-
   und Zero-Copy-Märkte komplett.
2. **OMG-Spec-Submission anstoßen** — 1-3 Jahre Vorlauf, blockiert
   uns nicht.
3. **Defacto-Standard übernehmen** (z.B. RTI FlatData) — Lock-in zu
   einem Konkurrenz-Vendor.
4. **Eigene Vendor-Spec mit voller PROCESS.md-Disziplin** (gewählt) —
   transparent, audit-fähig, kann später in OMG-Submission umgewandelt
   werden.

## Konsequenzen

**Positiv**:
- Spec-Lücken werden geschlossen ohne dass wir uns auf einen Vendor-
  Lock-in einlassen.
- PROCESS.md-Disziplin (Items + Repo + Tests) gilt einheitlich.
- Vendor-PIDs sind dokumentiert, Cross-Vendor-Compat bleibt erhalten.

**Negativ**:
- Caller, der zwei DDS-Vendoren parallel benutzt, sieht zwei FlatData-
  Patterns (RTI vs ZeroDDS).
- Wir tragen alleine den Pflegeaufwand für die Specs.

**Folge-Aufgaben**:
- ADR-0002 (async runtime-agnostic), ADR-0003 (flatdata Backend-Trait),
  ADR-0004 (Iceoryx2 optional), ADR-0005 (Dual-Stack opt-in),
  ADR-0006 (PID_SHM_LOCATOR).

## Referenzen

- `docs/specs/zerodds-async-1.0.md`
- `docs/specs/zerodds-flatdata-1.0.md`
- `docs/spec-coverage/PROCESS.md`
- OMG DDS-XTypes 1.3 §7.3.4 (vendor-specific extensions, allowed bits)
