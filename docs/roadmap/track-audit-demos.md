# Track RC2-C — Demo-Audit

**Goal:** alle Demos in `examples/demos/` werden hands-on getestet,
README-Schritte verifiziert, fehlende Pieces gebaut, Performance-Daten
neu erhoben. Jeder Demo bekommt eine Audit-Doku mit "ich habe das
selbst durchgespielt"-Sign-off.

**Status:** 📋 todo

**Estimate:** 2 Personenwochen.

## Demos im Audit-Scope

| Demo | Status laut Tracker | Audit-Pflicht |
|---|---|---|
| `examples/demos/dds-warehouse` | ✅ rc1-ready | hands-on durchspielen, alle 10 Stations laufen lassen, Orchestrator-Test grün |
| `examples/demos/perf-camera-dds` | 🔄 Skeleton | **Implementation fehlt** — Flutter-Publisher + Qt-Tileview noch nicht da, README-only. Code bauen oder Demo entfernen |
| `examples/demos/otel` | ✅ rc1-ready | hands-on: Jaeger-Compose, talker-Bin laufen, Spans im Jaeger-UI sehen |
| `examples/demos/shapes` | nicht im Tracker | Audit ob das die OMG-Shapes-Demo-Compat ist, README + run-script verifizieren |

## Pro Demo: Audit-Checklist

1. **Setup** — README.md von oben durchgehen, jeder Befehl ausgeführt
2. **Build** — alle Build-Schritte funktionieren auf Linux + macOS
3. **Run** — Demo läuft tatsächlich, sichtbarer Output
4. **Cross-Vendor** (wo relevant) — gegen Cyclone DDS
5. **Performance-Snapshot** — wenn die Demo Perf-Numbers zeigt, neu erheben
6. **Doc-Polish** — README aktualisiert, Bilder/Diagramme aktuell
7. **Spec-Referenzen** — Welche OMG-Specs spielen eine Rolle, sind die
   verlinkt
8. **Audit-Doku** — `examples/demos/<demo>/AUDIT.md` mit Datum + Reviewer

## Spezifisch: perf-camera-dds (DM.2)

Die Demo ist **Skeleton-only** — `flutter-publisher/` und `qt-tileview/`
enthalten nur READMEs, kein lauffähiger Code. Drei Optionen:

**Option A — voll bauen** (5-7 PT):
- Flutter-Publisher: native camera plugin, WebSocket-bridge to DDS,
  encode camera frames as DDS samples
- Qt-Tileview: subscribe topic, decode frames, render in Qt6 widget
- Performance-test: smartphone → DDS → Desktop tile-grid

**Option B — als reduzierten "concept-demo" akzeptieren** (1 PT):
- ARCHITECTURE.md + idl/camera.idl bleibt
- README sagt klar "concept-demo, implementation gated on Flutter
  ecosystem maturity"
- Tracker-Entry auf `🚫 deferred` setzen

**Option C — durch andere Demo ersetzen**:
- Statt camera: ein einfacherer "mobile sensor → DDS"-Demo (GPS, Accel)
- Schneller umzusetzen + weniger Flutter-Plugin-Komplexität

**Empfehlung:** Option B für RC2 (Skeleton honestly als concept
documenten + tracker auf deferred), Option A für post-1.0 wenn
ein User das wirklich braucht.

## Acceptance

- 4 Demos audited, jeweils mit `AUDIT.md`-Sign-off
- Tracker-Entries spiegeln den echten Stand
- perf-camera-dds nicht mehr in-review, sondern entweder ✅ ready oder
  🚫 deferred mit Decision-Record

## Dependencies

- Linux + macOS Test-Hosts (für hands-on Build/Run)
- Cyclone DDS 0.10.2 + omniORB als externe Counterparts (haben wir
  bereits in der Interop-Compose)

## Risks

- **Demos kaputt durch Drift** — wenn die Demos seit Phase-7-Build nicht
  mehr getestet wurden, könnten cargo-build-failures auftreten.
  Mitigation: jeder Demo bekommt einen CI-job `cargo check` als
  Smoke-Lever, läuft on-PR.
