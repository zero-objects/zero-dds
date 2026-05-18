# Roadmap und Phasen-Plan

> **Status:** Draft v0.2
> **Abhängigkeiten:** Alle vorangegangenen Dokumente

## 1 Grundannahmen

- **Ausführungs-Modell:** Bootstrap-vor-Expansion. Interner Proof zuerst, externe Partnerschaften danach.
- **Kern-Team (Bootstrap-Era):** 2–4 Senior-Engineers intern.
- **Claude-Teams-Augmentation:** durchgehend, realistisch 4–8× Throughput-Multiplier je nach Arbeitsbereich.
- **Externe Partnerschaft (Ferrous Systems):** erst in Expansion-Era, nach internem Proof und wenn externe Mittel mobilisiert.
- **OMG-Mitgliedschaft, Patent-Clearance, Community-Aufbau:** ebenfalls Expansion-Era.
- **Gesamt-Horizont bis interner Proof:** 10–14 Kalender-Monate.
- **Gesamt-Horizont bis externes Cert-Ready:** +18–24 Monate nach Proof, je nach Ressourcen.

## 2 Drei Eras auf einen Blick

| Era | Dauer | Team | Ziel |
|---|---|---|---|
| **Bootstrap-Era** (Phasen 0–2) | 10–14 Monate | 2–4 intern + Claude-Teams | Internes MVP, Stack trägt interne Zielanwendung |
| **Proof-Era** (Phasen 3–4) | 6–10 Monate | 3–5 intern + Claude-Teams | Benchmarks gegen eProsima/Cyclone bestanden, externe Readiness |
| **Expansion-Era** (Phasen 5+) | je nach Mitteln | 5+ intern + Partner | OMG, Ferrous, Cert, Community |

## 3 Phase 0: Foundation (Monate 1–2) · Bootstrap-Era

**Ziel:** Architektur ist abgenommen, Workspace-Skelett steht, erste technische Spikes bestanden. Fokus auf Code-Readiness, keine externen Aktivitäten.

### Ergebnisse

- Alle Architektur-Dokumente (00–07) reviewed und intern freigegeben
- Workspace-Skelett mit allen Crates (leer, aber CI-verdrahtet) im Repository
- CI-Pipeline: Build-Matrix für Full + Standard für Linux x86_64/ARM64
- Custom `zerodds-lint` Clippy-Plugin mit den wichtigsten Safety-Regeln (auch ohne sofortige Ferrocene-Integration)
- **IDL4-Parser (Grammar-driven)**: vollständiger OMG-IDL-4.2-Parser mit Earley-Engine,
  BNF als Laufzeit-Daten, Delta-Composition für Versions-/Vendor-Varianten,
  mindestens ein Vendor-Delta (RTI oder Cyclone) als Proof-of-Concept.
  Migrations-Fähigkeit ist explizites Sales-Argument gegenüber eProsima/RTI
  (siehe `07_risks_and_strategy.md §3.2`). Scope-Umfang und Milestones:
  `docs/rfcs/0001-idl-parser-architecture.md` und
  `.planning/wp-0.3-idl-parser/PLAN.md`
- CDR/XCDR1-Prototyp: serialisiert/deserialisiert die Basis-Typen aus dem XTypes-Spec-Annex
- UDP-Transport und allereinfachster Best-Effort-Writer/Reader kommunizieren zwischen zwei Test-Binaries
- Hello-World-Interop: ein selbstgebauter Writer publiziert an einen Cyclone-DDS-Subscriber erfolgreich

**Bewusst nicht in Phase 0:** OMG-Vendor-ID-Antrag, Ferrous-Systems-Engagement, Patent-Clearance, externe Ankündigungen. Diese Aktivitäten gehören in die Expansion-Era.

### Arbeitspakete

| Paket | Verantwortung | Claude-Teams-Nutzung |
|---|---|---|
| WP 0.1: Architektur-Review-Zyklus | Team intern, moderiert von Tech-Lead | Dokument-Kohärenz-Checks |
| WP 0.2: Workspace-Skelett + CI | Platform | Template-Generierung, CI-Config |
| WP 0.3: IDL4-Parser (Grammar-driven, voller Scope) | Protocol | Earley-Engine, BNF-Transkription, AST, Vendor-Delta-PoC. Siehe `docs/rfcs/0001-idl-parser-architecture.md` und `.planning/wp-0.3-idl-parser/PLAN.md`. **Scope-Change 2026-04-17:** erweitert von „Prototyp parst Samples" auf vollen IDL-4.2-Parser mit Migration-Fähigkeit; Dauer 5–7 Wochen statt 1–2 Wochen. **Status 2026-04-18: ABGESCHLOSSEN** (M1–M7 erreicht; 442 lib + Integration-Tests grün, RTI-Delta als PoC, `zerodds-idlc --parse-only` funktional; siehe `crates/idl/README.md`) |
| WP 0.4: CDR-Prototyp | Protocol | XCDR2 Encoder/Decoder (Primitives/Composite/Extensibility). **Status 2026-04-18: ABGESCHLOSSEN** (4-Wochen-Spike; 84 lib + 7 Integration-Tests; siehe `crates/cdr/README.md` und `.planning/wp-0.4-cdr-prototyp/PLAN.md`) |
| WP 0.5: UDP + Minimal-RTPS-Writer | Protocol | RTPS-Wire-Types + Submessages (DATA/HEARTBEAT/ACKNACK/GAP) + UDP-Transport + Best-Effort Writer/Reader. **Status 2026-04-18: ABGESCHLOSSEN** (4-Wochen-Spike; 88 zerodds-rtps + 6 zerodds-transport + 9 zerodds-transport-udp Tests; E2E write→UDP→read funktional; siehe `crates/rtps/README.md`) |
| WP 0.6: Cyclone-Interop-Harness | Platform | Wire-Compliance gegen Cyclone-DDS-Reference-Frames + Docker-compose-Harness fuer kuenftige Live-Captures. **Status 2026-04-18: ABGESCHLOSSEN** (2-Wochen-Spike; 11 Compliance-Tests; ZeroDDS-DATA-Bytes byte-identisch zu Cyclone-Layout. Live-Interop braucht Discovery aus Phase 1 — siehe `tests/interop/COMPLIANCE.md`) |
| WP 0.7-A: SPDP Live-Discovery (vorgezogen aus Phase 1) | Protocol | Multicast-UDP + ParameterList + ParticipantBuiltinTopicData + SpdpBeacon/Reader + Cache + Demo. **Status 2026-04-18: ABGESCHLOSSEN** (3-Wochen-Spike + 1-Tages-Polish; vorgezogen aus Phase 1, weil Live-Interop-Tests sonst nicht moeglich; 9 SPDP + 24 zerodds-rtps-neue-Tests; spdp_demo verifiziert E2E Loopback-Discovery; **LIVE-MULTI-VENDOR-INTEROP VERIFIZIERT**: ZeroDDS entdeckt Eclipse Cyclone DDS (vendor 0x0110) und eProsima Fast-DDS (vendor 0x010F) auf Debian 12 — siehe `tests/interop/INTEROP_REPORT.md`. SEDP + Reliable bleiben in Phase 1) |
| WP 0.7: `zerodds-lint`-Plugin (Safety-Regeln) | Quality | Lint-Regeln aus Safety-Spec. **Status 2026-04-18: ABGESCHLOSSEN** (Phase-0-Variante: AST-basiert auf stable Rust, kein Nightly/dylint. 6 Lints implementiert: `dds_require_safety_comment`, `dds_no_dyn_in_safe`, `dds_safety_classification_present`, `dds_no_panic_in_safe`, `dds_no_alloc_in_hot_path`, `dds_bounded_recursion`. CI-Job `zerodds-lint` aktiv. Bestand sweep-clean nach 22 Recursion-Markern in `crates/idl`. Echte Clippy-Plugin-Variante mit Type-Info → Phase 1) |

Interim vorläufige Vendor-ID aus dem OMG-Entwickler-Range wird verwendet, bis in Expansion-Era ein formaler Antrag gestellt wird.

### Risiken in Phase 0

- **Spec-Interpretationsstreit:** OMG-Specs sind an manchen Stellen mehrdeutig. Mitigation: referenzieren Cyclone DDS und Fast DDS als "existence proof" und dokumentieren Abweichungen.
- **Scope-Creep durch kleines Team:** mit 2–4 FTE müssen harte Prioritäten gesetzt werden. Mitigation: strenge Phase-Gate-Disziplin, keine Expansion-Era-Features vorziehen.
- **Claude-Teams-Abhängigkeit real:** bei sehr kleinem internen Team wird Claude-Augmentation zum kritischen Pfad. Mitigation: Pair-Programming-Pattern statt Solo-Agent-Generierung; menschlicher Review bei allen Protocol-Entscheidungen.

## 4 Phase 1: Protocol-Core (Monate 3–8) · Bootstrap-Era

**Ziel:** RTPS-Reliable-Protokoll komplett, Discovery funktioniert, XTypes-Kern implementiert. Fokus auf Korrektheit und Interop mit Cyclone DDS.

**Team:** 2–3 interne Engineers + Claude-Teams.

### Ergebnisse

- `zerodds-rtps`: Vollständiges RTPS 2.5 Wire-Protokoll
  - Reliable Writer/Reader State Machines
  - Heartbeats, Acknacks, Gaps, Fragmentation
  - Best-Effort-Pfad
  - Multi-Writer, Multi-Reader
- `zerodds-discovery`: SPDP + SEDP
- `zerodds-types`: XTypes 1.3 Kern (TypeLookup als Stretch-Goal, notfalls Phase 2)
- `zerodds-qos`: alle Standard-QoS-Policies, Request/Offered-Kompatibilität
- `zerodds-transport-tcp`: TCP-PSM funktional
- `zerodds-transport-shm`: Shared-Memory-Transport funktional (Linux)
- Interop-Tests in CI gegen Cyclone DDS und Fast DDS
- First internal release: `v0.1.0-alpha`

### Arbeitspakete

| Paket | Wochen | Claude-Teams-Leverage |
|---|---|---|
| WP 1.1: Reliable Protocol | 8 | Hoch für Boilerplate, niedrig für Korrektheits-Details |
| WP 1.2: Fragmentation | 3 | Mittel |
| WP 1.3: SPDP | 4 | Hoch — Built-in-Topic-Serialisierung |
| WP 1.4: SEDP | 5 | Hoch |
| WP 1.5: TypeLookup | 4 | Hoch |
| WP 1.6: XTypes Type Compatibility | 6 | Mittel — Regeln formal aus Spec ableitbar |
| WP 1.7: QoS Policies + Matrix | 4 | Hoch — Matrix ist Tabellen-basiert |
| WP 1.8: TCP Transport | 3 | Hoch |
| WP 1.9: Shared Memory Transport | 4 | Mittel |
| WP 1.10: Compliance-Test-Suite | 6 | Hoch — Tests aus Spec-Vektoren generierbar |
| WP 1.11: CI-Interop-Harness | 2 | Hoch |

### Meilensteine

- **M1.1 (Monat 4):** Reliable-Protokoll-Prototyp funktioniert zwischen zwei eigenen Nodes
- **M1.2 (Monat 5):** Discovery vollständig, erster Third-Party-Peer sieht uns in seiner Discovery
- **M1.3 (Monat 7):** XTypes mit Type-Evolution funktioniert
- **M1.4 (Monat 8):** CI läuft grün gegen Cyclone und Fast DDS auf Hello-World + Chat-Test

## 5 Phase 2: Bootstrap-MVP (Monate 9–13) · Bootstrap-Era

**Ziel:** Stack trägt interne Zielanwendung. DCPS-API produktiv, DDS-Security funktional, C/C++-Bindings komplett. Weitere Bindings verschoben auf Proof-Era.

**Team:** 3–4 interne Engineers + Claude-Teams.

### Ergebnisse

- `zerodds-dcps`: Vollständige DCPS-API in idiomatischem Rust
- `zerodds-security`: DDS-Security 1.2 Plugin-Framework + Built-in Auth/AC/Crypto
  - PKI-basierte Authentication (Ed25519, RSA, ECDSA) — **Standard-Suite als Default für Interop**
  - Permissions-Documents
  - AES-GCM + AES-GMAC für Data-Protection — **Standard-Suite als Default**
  - HSM-Support und EU-Crypto-Suites: Plugin-ready, konkrete Implementierung in späterer Phase
- `zerodds-sys`: Stabile C-ABI (eingefroren ab Ende dieser Phase)
- `zerodds-cpp`: C++17-Wrapper + IDL4-C++-Mapping funktional
- `zerodds-xml`: DDS-XML-Parser + QoS-Profile-Loader
- Interne Zielanwendung läuft auf ZeroDDS mit echter Multi-Node-Topologie
- Internal release: `v0.2.0-mvp`

**Bewusst nicht in Phase 2:** C#-, Java-, Python-Bindings; OMG Plug-Fest-Teilnahme; externe Pilot-User. Alles Proof-Era.

### Arbeitspakete

| Paket | Wochen | Claude-Teams-Leverage |
|---|---|---|
| WP 2.1: DCPS Public-API | 6 | Hoch |
| WP 2.2: DDS-Security Auth-Plugin (Standard-Suite) | 5 | Mittel — Crypto-Details brauchen Reviews |
| WP 2.3: DDS-Security AC-Plugin | 4 | Hoch |
| WP 2.4: DDS-Security Crypto-Plugin (Standard-Suite) | 4 | Mittel |
| WP 2.5: C-ABI Design + Freeze | 3 | Mittel — muss stabil sein |
| WP 2.6: C++-Binding + IDL4-C++ | 5 | Hoch |
| WP 2.7: DDS-XML | 3 | Hoch |
| WP 2.8: Interne Anwendungs-Integration | 4 | Mittel |

### Meilensteine

- **M2.1 (Monat 10):** Rust-DCPS-API voll funktional
- **M2.2 (Monat 11):** Security-Auth-Flow gegen Cyclone-DDS-Security-Peer erfolgreich
- **M2.3 (Monat 12):** C/C++-Bindings kompilieren und Hello-World läuft
- **M2.4 (Monat 13):** **Bootstrap-Proof erreicht** — interne Anwendung läuft auf ZeroDDS

## 6 Phase 3: Internal Hardening und Tooling (Monate 14–17) · Proof-Era

**Ziel:** ZeroDDS in interner Anwendung gehärtet, Tooling und Observability auf produktivem Niveau für eigene Betriebs-Zwecke.

**Team:** 3–4 interne Engineers + Claude-Teams.

### Ergebnisse

- Internal-Deployment-Hardening: Chaos-Testing gegen echte Netzwerke, Packet-Loss-Szenarien, Reboot-Tests
- `zerodds-recorder`: deterministisches Recording + Replay (wichtig für interne Post-Mortems und Regression)
- `zerodds-monitor`: OpenTelemetry-Instrumentierung durchgehend
- `zerodds-dashboard`: Tauri-App mit Discovery-Graph, Metriken, Replay-Browser
- `zerodds-admin`: Admin-CLI
- `zerodds-perf`: Load-Generator + Benchmark-Suite
- Erste Benchmark-Messungen gegen eProsima Fast DDS und Cyclone DDS auf demselben Test-Setup
- Internal release: `v0.3.0`

### Arbeitspakete

| Paket | Wochen | Claude-Teams-Leverage |
|---|---|---|
| WP 3.1: Chaos-Test-Suite | 4 | Hoch |
| WP 3.2: Recording-Format + Recorder | 4 | Hoch |
| WP 3.3: Replay-Engine | 4 | Hoch |
| WP 3.4: OTel-Instrumentierung | 3 | Sehr hoch — mechanisch |
| WP 3.5: Metrik-Katalog + Exporter | 2 | Sehr hoch |
| WP 3.6: Tauri-Dashboard UI | 6 | Hoch |
| WP 3.7: Admin-CLI | 2 | Hoch |
| WP 3.8: Performance-Benchmark-Harness + eProsima-Comparison | 3 | Mittel |
| WP 3.9: Internal-Anwendungs-Härtung | 4 | Niedrig — echte Bugs brauchen menschliches Debugging |

### Meilensteine

- **M3.1 (Monat 15):** Dashboard zeigt Live-Discovery-Graph aus echtem internen System
- **M3.2 (Monat 16):** Erste Benchmarks gegen eProsima, Gap quantifiziert
- **M3.3 (Monat 17):** Stack ist Produktions-tauglich für interne Anwendung ohne Safety-Zwang

## 7 Phase 4: External-Readiness (Monate 18–22) · Proof-Era

**Ziel:** Stack ist ready für externe Sichtbarkeit. Weitere Bindings, XRCE, PlatformIO, Performance-Parität gegen eProsima.

**Team:** 4–5 interne Engineers + Claude-Teams. Externe Expertise punktuell (Performance-Consulting, Cross-Compile-Unterstützung).

### Ergebnisse

- Remaining Bindings: C# (NuGet + NativeAOT), Java (Pure-Java DDS-Java-PSM), Python (PyO3) alle funktional mit IDL4-Mappings
- `zerodds-rpc`: DDS-RPC 1.0 Framework komplett
- `zerodds-xrce-client` + `zerodds-xrce-agent`: Micro-Profile für Cortex-M33/M7/M4, ESP32, STM32
- PlatformIO-Library-Repository mit Beispielen für ESP32 und STM32
- Performance-Hardening: Zero-Copy-SHM-Pfade, TSN-optional
- Performance-Parität mit eProsima Fast DDS innerhalb ±20% auf Referenz-Hardware
- Stress- und Chaos-Tests: Netzwerk-Partitionen, Packet-Loss, Clock-Skew
- Dokumentation konsolidiert: User Guide, Developer Guide, Operator Guide
- First potentially-public release: `v1.0.0-rc` (Entscheidung über Öffentlichkeit am Phase-Ende)

### Arbeitspakete

| Paket | Wochen | Priorität |
|---|---|---|
| WP 4.1: C#-Binding + IDL4-C# | 5 | Hoch |
| WP 4.2: Java-Binding + IDL4-Java | 5 | Hoch |
| WP 4.3: Python-Binding | 3 | Mittel |
| WP 4.4: DDS-RPC | 4 | Mittel |
| WP 4.5: XRCE-Client + Agent | 6 | Hoch |
| WP 4.6: PlatformIO-Package + Examples | 4 | Hoch |
| WP 4.7: Zero-Copy-SHM-Pfad | 4 | Hoch |
| WP 4.8: Chaos- und Performance-Test-Suite | 4 | Hoch |
| WP 4.9: Dokumentations-Konsolidierung | 4 | Hoch |

### Meilensteine

- **M4.1 (Monat 20):** Alle sechs Bindings funktional
- **M4.2 (Monat 21):** XRCE-Client läuft auf ESP32 und STM32, PlatformIO-Alpha
- **M4.3 (Monat 22):** **Proof-Era abgeschlossen** — Stack ist Production-tauglich und benchmark-kompetitiv. Entscheidung über Expansion-Era kann informed getroffen werden.

## 8 Phase 5+: Expansion-Era (bedingt) · ab ca. Monat 22

Die Expansion-Era aktiviert sich **nach** bestätigtem Proof und **sobald** externe Mittel mobilisiert werden können. Die Aktivitäten sind konditional und zeitlich entkoppelt; jede kann unabhängig anlaufen, wenn Ressourcen und strategische Bedingungen stimmen.

### 8.1 Expansion-Aktivitäts-Tracks

**Track A: OMG-Ecosystem-Integration**
- OMG-Mitgliedschaft (Contributing oder höher, je nach Budget)
- OMG-Vendor-ID-Antrag formal
- OMG Plug-Fest-Teilnahme
- Cross-Vendor-Interop-Zertifizierung
- Aufwand: 3–6 Monate Kalenderzeit, 1–2 FTE plus Membership-Fees

**Track B: Ferrous-Systems-Partnerschaft und Ferrocene-Safe-Profile**
- Engineering-Partnerschaft mit Ferrous Systems
- Ferrocene-Integration in CI für Safe-Profile-Builds
- Target-Port-Engineering für QNX/INTEGRITY (falls Kunden-Nachfrage)
- Aufwand: 6–9 Monate, 1–2 FTE intern plus externe Stunden

**Track C: Safety-Zertifizierung**
- Formale Requirements-Extraktion und Traceability-Matrix-Konsolidierung
- MC/DC-Coverage-Push für Safe-Subset
- Safety Manual
- Externer Audit (TÜV Süd oder Alternative)
- Ziel-Standards: ISO 26262 ASIL D, IEC 61508 SIL 3, IEC 62304 Class C, DO-178C DAL B audit-ready
- Aufwand: 6–10 Monate, 2–3 FTE + externe Auditoren
- Kosten: mittlerer sechsstelliger Bereich für Audit-Gebühren

**Track D: Patent-Clearance und Legal**
- Patent-Anwalt-Engagement für Freedom-to-Operate-Analyse
- Trademark-Recherche und -Registrierung für "ZeroDDS"
- EU-Dual-Use-Klassifizierung (BAFA)
- Cyber Resilience Act Compliance-Assessment
- Aufwand: 2–4 Monate, 0.5 FTE intern plus externe Kanzlei

**Track E: Community und Open-Source-Release**
- Public GitHub-Release (wenn strategisch gewollt)
- Governance-Modell (wenn Donation an Foundation angestrebt)
- Community-Management-Rolle
- Conference-Präsenz (ROSCon, Embedded World, Eclipse Conference)
- Aufwand: kontinuierlich, 0.5–1 FTE

### 8.2 Empfohlene Track-Priorisierung

Reihenfolge wenn Mittel limitiert sind:
1. Track D (Trademark-Schutz für ZeroDDS-Name) — geringste Kosten, höchstes Schutzwert
2. Track A (OMG) — etabliert externe Glaubwürdigkeit
3. Track B (Ferrous) — ermöglicht Track C
4. Track C (Safety) — erschließt Markt-Segmente
5. Track E (Community) — parallel zu A/C je nach Release-Strategie

## 9 Post-v1.1 Roadmap (Ausblick)

Nach der initialen Zertifizierung sind folgende Weiterentwicklungen vorgesehen:

- **Post-Quantum-Crypto** als Security-Plugin-Option (hybride Crypto-Suites)
- **DO-178C DAL A** Vollzertifizierung (falls Kunden-Nachfrage vorhanden)
- **Additional RTOS-Ports:** Green Hills INTEGRITY-178, LynxOS-178 Safety-Partitionen
- **Additional Sprach-Bindings:** Go, JavaScript/TypeScript via Deno, falls Community-Nachfrage
- **WebDDS-Brücke** für Web-UI-Integration
- **Web-basiertes Dashboard** als Ergänzung zur Tauri-Desktop-App
- **FDA-Unterstützung** für medizinische Use Cases (IEC 62304 Class C Pakete)

## 10 Resource-Ramp-Übersicht (Bootstrap + Proof)

| Monat | FTE intern | Phase / Era | Hauptaktivität |
|---|---|---|---|
| 1 | 2 | 0 — Bootstrap | Architektur-Finalisierung |
| 2 | 2–3 | 0 — Bootstrap | Spikes, Skelett |
| 3–4 | 3 | 1 — Bootstrap | RTPS-Reliable-Start |
| 5–6 | 3 | 1 — Bootstrap | Discovery, XTypes |
| 7–8 | 3 | 1 — Bootstrap | QoS, Interop |
| 9–10 | 3–4 | 2 — Bootstrap | DCPS, Security-Standard |
| 11–12 | 3–4 | 2 — Bootstrap | C/C++-Bindings, XML |
| 13 | 3–4 | 2 — Bootstrap | **Proof-Point: interne Anwendung läuft** |
| 14–15 | 3–4 | 3 — Proof | Hardening, Tooling |
| 16–17 | 4 | 3 — Proof | Dashboard, Benchmarks |
| 18–20 | 4–5 | 4 — Proof | Weitere Bindings, XRCE, PlatformIO |
| 21–22 | 4–5 | 4 — Proof | Performance-Parität, Docs, v1.0-rc |

**Kumulierter interner Personal-Einsatz bis Ende Proof-Era:** ca. 75–85 Personen-Monate = ~6–7 Personen-Jahre. Claude-Teams-Multiplikator von ~4–6× entspricht klassischem Aufwand von 25–40 Personen-Jahren.

**Expansion-Era** ist nicht in dieser Tabelle enthalten, da Ramp und Zeitrahmen von Track-Entscheidungen und externer Mittel-Verfügbarkeit abhängen.

## 11 Frühe Validierungs-Punkte

Der Stack wird nicht erst nach 2+ Jahren validiert, sondern an klaren Checkpoints:

| Monat | Validierung |
|---|---|
| 4 | Technologie-Wahl (Rust) trägt — Go/No-Go für Architektur |
| 8 | Interop mit Cyclone und Fast DDS funktioniert — Go/No-Go für MVP-Push |
| 13 | **Bootstrap-Proof:** interne Anwendung läuft auf ZeroDDS |
| 17 | Benchmark-Gap zu eProsima quantifiziert — informiert Performance-Tuning |
| 22 | **Proof-Era abgeschlossen:** v1.0-rc ready, Entscheidung über Expansion-Era |
| ~30 | Cert-ready (wenn Track B+C aktiviert wurden) |

Jeder Checkpoint kann zu Kurs-Korrektur führen, einschließlich Pause/Re-Scope/Kill-Entscheidung wenn grundlegende Annahmen brechen.
