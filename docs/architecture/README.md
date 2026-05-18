# ZeroDDS — Architektur-Dokumentation

Diese Dokument-Suite bildet das architektonische Fundament für die Entwicklung von **ZeroDDS** — einer souveränen, vollständigen DDS-Implementierung in Rust mit Bindings für C, C++, C#, Java, Python und Rust.

Der Name ZeroDDS reflektiert das Kernversprechen: _zero dependencies_ (Safe-Core), _zero panic_ (Vertrag), _zero unsafe_ (wo strukturell möglich), _zero copy_ (SHM-Pfad), _zero vendor lock-in_.

## Ausführungs-Modell

ZeroDDS wird als **internes Core-Projekt** entwickelt (Apache 2.0-lizenziert, Optionalität für spätere Öffnung gewahrt). Wir sind unser erster Kunde: der Stack wird gegen eine konkrete interne Anwendung validiert, bevor externe Partnerschaften, OMG-Membership, Patent-Clearance oder Safety-Zertifizierung aktiviert werden.

**Bootstrap-Era** (Phasen 0–2, ~10–14 Monate): Internes MVP.
**Proof-Era** (Phasen 3–4, ~6–10 Monate): Benchmark-Parität mit eProsima, External-Readiness.
**Expansion-Era** (Phase 5+, konditional): OMG, Ferrous Systems, Safety-Audit, Community.

## Lese-Reihenfolge nach Rolle

### Für Leadership und Stakeholder
1. `00_overview.md` — Executive Summary, Mission, Erfolgskriterien

### Für Tech-Leads und Architekten
1. `00_overview.md`
2. `01_scope_and_specs.md` — OMG-Spec-Coverage
3. `02_architecture.md` — System- und Crate-Architektur
4. `03_profiles_and_platforms.md` — Deployment-Profile
5. `07_risks_and_strategy.md` — Strategische Risiken

### Für Safety-Engineers
1. `00_overview.md`
2. `02_architecture.md`
3. `04_safety_by_architecture.md` — Safe-Subset-Vertrag (primäres Dokument)
4. `03_profiles_and_platforms.md` — Safe-Profile-Details
5. `06_roadmap.md` §8 — Phase-5 Audit-Pfad

### Für Product-Engineers (pro Sub-System)
1. `02_architecture.md` — eigene Crate-Verantwortung identifizieren
2. `01_scope_and_specs.md` — welche OMG-Specs betreffen dich
3. `04_safety_by_architecture.md` — Coding-Regeln (wenn Safe-Crate)
4. `05_observability_and_tooling.md` — Instrumentierungs-Erwartungen

### Für Platform/DevOps
1. `03_profiles_and_platforms.md` — Build-Matrix
2. `05_observability_and_tooling.md` — Monitoring-Design
3. `06_roadmap.md` — Release-Plan

## Dokument-Übersicht

| Datei | Umfang | Review-Intervall |
|---|---|---|
| `00_overview.md` | Mission und Vision | Quartalsweise |
| `01_scope_and_specs.md` | OMG-Spec-Coverage und Conformance | Pro Release |
| `02_architecture.md` | System- und Crate-Architektur | Quartalsweise |
| `03_profiles_and_platforms.md` | Profile und Plattform-Matrix | Pro Release |
| `04_safety_by_architecture.md` | Safety-Vertrag | Phasenweise oder bei Regel-Änderungen |
| `05_observability_and_tooling.md` | Observability und Tooling | Quartalsweise |
| `06_roadmap.md` | Phasen und Meilensteine | Monatlich |
| `07_risks_and_strategy.md` | Risiken und strategische Optionen | Monatlich |
| `08_heterogeneous_security.md` | Per-Peer/Per-Interface-Security für System-of-Systems (WP 4.9) | Pro Release |
| `09_delegation.md` | Gateway/Bridge-Delegation für Vehicle-Mesh + Edge-Peers ohne eigenen Cert (WP 4H-j) | Pro Release |

## Änderungs-Prozess

Alle Architektur-Dokumente liegen im Haupt-Repository unter `docs/architecture/`. Änderungen erfolgen via Pull Request und erfordern:
- Mindestens ein Review durch Tech-Lead oder Safety-Engineer (bei `04_safety_by_architecture.md`)
- Begründung im PR-Beschreibung
- Update des betreffenden `Status:`-Feldes und ggf. Versions-Inkrement

## Verbundene Ressourcen (außerhalb dieser Suite)

- Standards-Registry unter `docs/standards/` für alle externen Specs (OMG, W3C, IETF, CNCF, OASIS, ISO/IEC), auf denen ZeroDDS aufbaut
- Requirements-Tracker (Polarion, DOORS, oder GitHub Issues mit `REQ-...`-Labels)
- ADR-Verzeichnis unter `docs/adr/` für Architecture Decision Records
- Safety-Waiver-Register unter `docs/safety-waivers/`
- Design-RFCs unter `docs/rfcs/` für größere technische Vorschläge
- Security-Advisories unter `docs/security/advisories/`

## Konventionen

- **Sprache:** Deutsch für interne Dokumente, Englisch für Public-Facing (GitHub README, Spec-Files, Public API Docs).
- **Format:** Markdown, Commonmark + GitHub-Flavored Markdown Features.
- **Cross-Referenzen:** relativ (`02_architecture.md §3.1`) nicht absolut.
- **Terminologie:** englische technische Fachbegriffe werden nicht eingedeutscht (Crate, Workspace, Feature Flag, Discovery).
- **Code-Beispiele:** Rust als Hauptsprache, mit Syntax-Highlighting.

## Status

Dieses Dokument-Set befindet sich im **Draft v0.2**-Zustand (aktualisiert für ZeroDDS-Namen und Bootstrap-vor-Expansion-Strategie). Es wird in Phase 0 des Projekts formell reviewt, angepasst und freigegeben.
