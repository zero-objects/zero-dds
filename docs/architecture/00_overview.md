# ZeroDDS — Architektur-Überblick

> **Status:** Draft v0.2 · **Zielgruppe:** Engineering, Leadership, Stakeholder
> **Nächste Review:** nach Phase-0-Kickoff

## 1 Mission

**ZeroDDS** ist eine vollständige, souveräne DDS-Implementierung (OMG Data Distribution Service) in Rust mit Bindings für C, C++, C#, Java, Python und Rust. Der Name reflektiert das architektonische Kernversprechen:

- **Zero external dependencies** im Safe-Core (nur `core` + `alloc` + kuratierte no_std-Crates)
- **Zero panic** in allen Nicht-Comfort-Crates (Clippy-durchgesetzt)
- **Zero unsafe** wo strukturell möglich, jeder Ausnahme-Block mit SAFETY-Kommentar
- **Zero copy** im Shared-Memory-Transport-Pfad
- **Zero vendor lock-in** durch strikt offene Standards und Apache-2.0-Lizenz

Ziel ist eine Alternative zu bestehenden kommerziellen und Open-Source-DDS-Anbietern, die die für unsere Anwendungsfälle kritischen Lücken schließt: Souveränität der Lieferkette, Safety-Qualifizierbarkeit, moderne Observability und tiefe Embedded-Integration.

## 1.1 Ausführungsmodell: Bootstrap vor Expansion

ZeroDDS wird als internes Core-Projekt entwickelt. Wir sind unser erster Kunde — der Stack wird gegen eine konkrete interne Anwendung (verteiltes Sensor- und Entscheidungs-System auf Jetson-Thor-Plattform) validiert. Externe Positionierung, OMG-Mitgliedschaft, Patent-Clearance, Safety-Zertifizierung und Community-Aufbau folgen erst, wenn ein stabiles Basis-System existiert, das in internen Benchmarks gegen eProsima Fast DDS und Eclipse Cyclone DDS bestehen kann.

Die Begründung ist nüchtern: externe Partnerschaften, Audit-Budgets und Certifications lassen sich erst mobilisieren, wenn der interne Proof existiert. Umgekehrt wäre es ineffizient.

## 2 Strategische Begründung

Die aktuelle DDS-Landschaft zwingt zu unakzeptablen Kompromissen:

| Pain-Punkt | Heutige Realität | Unsere Antwort |
|---|---|---|
| **Transatlantische Abhängigkeit** | RTI (US, ITAR/EAR-exponiert), OpenDDS (US) dominieren Safety-Segment | EU-basierte Entwicklung, souveräne Lieferkette, keine Export-Kontroll-Risiken |
| **Safety-Pfad** | Nur RTI Connext Cert und eProsima Safe DDS bieten Cert-Evidence; beide teuer oder jung | Safety-by-Architecture ab Tag 1, Ferrocene-basierter Cert-Pfad zu ISO 26262 ASIL D und DO-178C |
| **Security** | DDS-Security 1.1/1.2 meist implementiert, aber keine Post-Quantum-Crypto, keine EU-Crypto-Suiten | Plugin-basierte Crypto, austauschbare Suites, Post-Quantum-ready |
| **Performance-Tooling** | Bestenfalls proprietäre Admin-Tools, kaum OpenTelemetry-Integration | Native OTel-Instrumentierung, W3C Trace Context, deterministisches Replay |
| **Embedded/MCU** | Fragmentiert: eProsima Micro XRCE-DDS für micro-ROS, RTI Micro separat | Einheitliche Codebasis, XRCE-Client für Cortex-M mit PlatformIO-Integration |
| **Lizenz-Exposure** | Kommerzielle Vendoren mit pro-Unit-Lizenz, proprietäre Source-Basis | Offene Lizenz-Option, Single-Vendor-Lock-In vermeidbar |

## 3 Kern-Eigenschaften der Ziel-Architektur

- **Spec-Konformität:** Vollständige OMG DDS Spec-Family (DCPS 1.4, RTPS 2.5, XTypes 1.3, Security 1.2, RPC 1.0, XML 1.0, XRCE 1.0) plus IDL4 mit Mappings nach C, C++, C#, Java, Python, Rust.
- **Vier Deployment-Profile** aus einer Codebasis: Full (Desktop/Server), Standard (Embedded Linux/RTOS), Safe (zertifizierbar), Micro (Cortex-M via XRCE).
- **Sechs Sprach-Bindings:** C, C++, C#, Java, Python, Rust — alle mit IDL4-Mapping.
- **Plattform-Coverage:** Linux x86_64/ARM64, Windows, macOS, QNX Neutrino, VxWorks, INTEGRITY, PikeOS, Deos, Zephyr, FreeRTOS, ESP-IDF, STM32Cube, bare-metal Cortex-M.
- **Safety-ready:** Safe-Subset-Crates sind no_std, no-panic, no-dynamic-alloc, Ferrocene-only. Audit-Pfad zu ISO 26262 ASIL D, DO-178C DAL B+, IEC 61508 SIL 3+ vorgesehen.
- **Observability-first:** OpenTelemetry-Instrumentierung durchgehend, Prometheus-Metriken, deterministisches Wire-Recording mit Replay, Tauri-basiertes Live-Dashboard.
- **PlatformIO-nativ:** Embedded-Distribution als PlatformIO-Library mit vorgefertigten Targets für die gängigen Framework-Stacks.

## 4 Erfolgskriterien

Erfolg wird in zwei Stufen gemessen, entsprechend der Bootstrap-vor-Expansion-Strategie.

### 4.1 Bootstrap-Proof-Kriterien (intern)

Der Stack gilt als intern-proven, wenn folgende Kriterien erfüllt sind:

1. RTPS-Reliable-Protokoll implementiert und in Interop-Tests mit Cyclone DDS und Fast DDS erfolgreich validiert.
2. DCPS 1.4 Minimum Profile plus Ownership und Content-Subscription Profile funktional.
3. DDS-Security 1.2 mit der Standard-Builtin-Plugin-Suite (AES-GCM, RSA/ECDSA) lauffähig und Interop-validiert.
4. C- und C++-Bindings funktional, IDL4-Mapping für diese beiden Sprachen komplett.
5. Kern-Anwendung (interner Use-Case) läuft produktiv auf ZeroDDS, löst klassischen Pub-Sub-Anforderungen der Anwendung.
6. Latency und Throughput auf Referenz-Hardware (ARM Jetson-Klasse und x86_64-Server) innerhalb von ±30% der eProsima-Fast-DDS-Werte auf demselben Test-Setup.

### 4.2 Expansion-Kriterien (extern, wenn Mittel verfügbar)

Nach bestätigtem internen Proof:

1. Interop-Zertifizierung am OMG Plug-Fest mit mindestens RTI Connext, Cyclone DDS, Fast DDS.
2. Vollständige sechs Sprach-Bindings funktional, alle IDL4-Mappings validiert.
3. XRCE-Client auf mindestens drei Embedded-Plattformen lauffähig.
4. Observability-Stack komplett: OpenTelemetry-Emission, Prometheus-Exporter, Wire-Recorder, Tauri-Dashboard.
5. Safety-Audit-Readiness für den Safe-Subset durch externen Auditor bestätigt.
6. Latency und Durchsatz innerhalb von ±20% der Spitzenwerte etablierter Vendoren.

## 5 Explizite Non-Goals

Bewusste Einschränkungen, die wir aus Scope halten, um Fokus zu wahren:

- **DLRL (Data Local Reconstruction Layer):** Der OMG-Spec-Teil ist weitgehend verwaist, keine relevanten Deployments. Wir implementieren ihn nicht.
- **CORBA-Interop:** Historisches Erbe. Keine Kunden-Nachfrage in unserem Zielsegment.
- **Proprietäre Wire-Erweiterungen:** Wir bleiben strikt bei RTPS-Standard, keine Vendor-spezifischen Erweiterungen wie RTI-FlatData oder OpenSplice-DDSI2E.
- **Legacy-Sprach-Bindings:** Ada, Fortran, JavaScript, Go stehen nicht im initialen Scope.
- **Kommerzielles SaaS-Management-Plane:** Cloud-gehostete Admin-Tools sind kein Ziel. Observability ist on-premises oder via Customer-Cloud.

## 6 Projektstruktur auf einen Blick

- **Kern-Team (Bootstrap-Phase):** 2–4 Senior-Engineers im internen Core-Team. Kein External-Hiring-Ramp bis interner Proof erreicht ist.
- **Claude-Teams-Augmentation:** durchgehend, realistisch 4–8× Acceleration je nach Arbeitsbereich. Bei kleinem Kern-Team ist Claude-Teams der primäre Force-Multiplier.
- **Externe Partnerschaften:** verschoben auf Post-Proof-Phase. Ferrous Systems, OMG-Mitgliedschaft, Patent-Anwalts-Engagement und Community-Aufbau werden aktiviert, wenn das Basis-System internally-proven ist und externe Mittel zur Verfügung stehen.
- **Governance:** internes Core-Projekt der Sponsor-Firma. Kein Foundation-Modell, kein externes Governance-Framework. Apache-2.0-Lizenz gewählt, um spätere Optionalität zu wahren (Donation an eine Foundation bleibt möglich, erfordert aber keine Planung jetzt).
- **Lizenz:** Apache 2.0 (entschieden).
- **Zeit-Horizont:** Bootstrap-Phase 10–14 Monate bis MVP mit interner Anwendung; danach iterative Weiter-Entwicklung je nach Ressourcen-Verfügbarkeit.

## 7 Dokumentations-Suite

Die folgenden Dokumente bilden zusammen das architektonische Fundament:

| # | Dokument | Zweck |
|---|---|---|
| 00 | `00_overview.md` | Dieses Dokument — strategische Mission |
| 01 | `01_scope_and_specs.md` | OMG-Spec-Coverage und Conformance-Ziele |
| 02 | `02_architecture.md` | System-Architektur und Crate-Workspace |
| 03 | `03_profiles_and_platforms.md` | Vier Profile, Plattform-Matrix, Binding-Matrix |
| 04 | `04_safety_by_architecture.md` | Safe-Subset-Vertrag und CI-Durchsetzung |
| 05 | `05_observability_and_tooling.md` | Live-Insights, Recording, Replay, UI |
| 06 | `06_roadmap.md` | Phasen-Plan, Meilensteine, Ressourcen |
| 07 | `07_risks_and_strategy.md` | Patent, IP, Community, Wettbewerbs-Antwort |

Jedes dieser Dokumente ist eigenständig lesbar und kann parallel von verschiedenen Stakeholder-Gruppen genutzt werden. Bei Änderungen ist Cross-Reference-Konsistenz zu wahren — Claude-Teams-unterstützbares Pattern.
