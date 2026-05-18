# Risiken und strategische Überlegungen

> **Status:** Draft v0.2
> **Abhängigkeiten:** Alle vorangegangenen Dokumente

Dieses Dokument identifiziert strategische und technische Risiken des Projekts und dokumentiert Mitigation-Strategien. Es wird periodisch reviewt, insbesondere an Phasen-Übergängen.

## 1 Technische Risiken

### 1.1 RTPS-Reliable-Protokoll-Bugs

**Risiko:** Das RTPS-Reliable-Protokoll ist eines der subtilsten Stücke im DDS-Stack. Fast DDS und Cyclone DDS haben über Jahre Bugs in diesem Bereich gefunden. Fehler zeigen sich oft erst unter Netzwerk-Stress, bei hoher Fragmentierungs-Dichte, oder in spezifischen Race-Conditions zwischen Heartbeats, Acknacks und Datenflüssen.

**Impact:** Produktions-Bugs in Reliable-Zustellung führen zu Daten-Verlust, Daten-Duplikation oder Deadlocks. Das ist in Mission-Critical-Kontexten inakzeptabel.

**Mitigation:**
- Property-Based-Testing und Model-Checking (Kani) ab Tag 1 für State Machines
- Fuzz-Testing der Wire-Parser (mindestens Nightly, 1h pro Run)
- Umfassende Interop-Tests gegen mindestens drei unabhängige Peer-Implementierungen
- Chaos-Engineering mit Netzwerk-Störung (Packet-Loss, Reorder, Delay, Partition)
- Code-Review-Pflicht durch mindestens zwei Senior-Engineers für alle Reliable-Protokoll-Änderungen
- Reference-Implementierung: Cyclone DDS als informelle Orientierung, mit dokumentierten Abweichungen

### 1.2 XTypes-Interoperabilität

**Risiko:** XTypes-Implementierungen variieren historisch zwischen Vendoren. Interop-Probleme, besonders bei komplexeren Type-Mutationen (appendable, mutable), sind verbreitet.

**Impact:** Enden-Nodes können sich nicht finden, obwohl Types "eigentlich" kompatibel sind. Das ist ein häufiger Support-Fall bei allen DDS-Vendoren.

**Mitigation:**
- Strikte Einhaltung der XTypes 1.3-Spec
- Interop-Tests pro Type-Mutation-Pattern (alle @final/@appendable/@mutable-Kombinationen)
- Dokumentierte Abweichungen von anderen Vendoren (wenn wir spec-konform sind, sie nicht)
- Type-Lookup-Service als Fallback bei unbekannten Typen

### 1.3 Performance-Parität

**Risiko:** Etablierte Vendoren haben 10–20 Jahre Performance-Tuning hinter sich. Eine neue Implementierung könnte in Benchmarks unterliegen, was Verkaufs-Argumente und Kunden-Akzeptanz beeinträchtigt.

**Impact:** Kunden wählen etablierten Vendor trotz Souveränitäts-Argumenten, wenn Performance-Gap zu groß ist.

**Mitigation:**
- Frühe Benchmark-Baseline (ab Phase 1) gegen Cyclone DDS und Fast DDS
- Performance-Regression-Tests in CI mit 5%-Schwelle
- Zero-Copy-SHM als Priorität in Phase 4
- Profile-guided Optimization (PGO) für Release-Builds
- Flamegraph-basierte Hot-Path-Analyse kontinuierlich
- Rust gibt uns strukturelle Vorteile (zero-cost abstractions, keine GC-Pauses); das sollte mindestens zu Parität führen

### 1.4 Ferrocene-Target-Lücke (verschoben auf Expansion-Era)

**Risiko:** Ferrocene ist aktuell für eine begrenzte Target-Menge qualifiziert. Wenn Kunden exotische Targets brauchen, könnte der Qualifications-Scope von Ferrocene nicht ausreichen.

**Impact:** Safe-Profile funktioniert auf Kunden-Hardware nicht oder nur unter Zusatzaufwand.

**Aktueller Status:** In Bootstrap- und Proof-Era verwenden wir stable Rust. Safe-Subset-Crates werden safety-ready geschrieben (siehe `04_safety_by_architecture.md`), aber ohne Ferrocene-Qualifikation gebaut. Ferrocene-Integration ist ein Track-B-Thema (siehe `06_roadmap.md` §8.1).

**Mitigation für später:**
- Early-Alignment mit Ferrous Systems über Ziel-Targets beim Start von Track B
- Contract-Engineering für Target-Ports (Ferrous bietet das als Service)
- Dual-Path-Strategie: Safe-Profile primär auf Mainstream-Targets (Linux x86_64/ARM64, QNX Neutrino), exotische Targets nur auf Kunden-Nachfrage

### 1.5 Claude-Teams-Produktivitäts-Enttäuschung

**Risiko:** Der geplante Multiplikator von 4–8× wird nicht erreicht, weil bestimmte Aufgaben (RTPS-Debugging, Interop-Issues, Performance-Tuning) nicht durch LLM-Augmentation beschleunigt werden.

**Impact:** Roadmap rutscht, Kosten steigen.

**Mitigation:**
- Klare Buckets pro Aufgabentyp mit realistischen Multiplikator-Erwartungen (siehe `06_roadmap.md`)
- Kontinuierliche Metrik: Welche Tasks liefen wie schnell mit vs. ohne AI-Augmentation?
- Kein Vertrauen auf 100×-Multiplikator; Plan konservativ kalkuliert
- Fallback-Puffer in Phase 4 und 5 eingeplant

## 2 Lizenzrechtliche und Patent-Risiken

### 2.1 RTI-Patent-Exposure

**Risiko:** RTI hält diverse Patente um DDS-Performance-Techniken (insbesondere Shared-Memory, FlatData, spezifische Discovery-Optimierungen). Eine eigene Implementierung kann versehentlich Patente berühren.

**Impact:** Patent-Klage, Lizenz-Zwang, oder Feature-Rücknahme. Sogar die bloße Drohung kann Investoren und Kunden abschrecken.

**Mitigation:**
- **Phase-0-Patent-Clearance** durch spezialisierten Anwalt mit IP-Erfahrung
- RTI-Patent-Portfolio systematisch kartieren
- Design-Around bei identifizierten Patenten
- Freedom-to-Operate-Gutachten vor Public-Release
- OMG-Spec-Features sind Royalty-Free (OMG Essential Claims) — darauf stützen, wo möglich
- Partnership-Option: Prior-Art-Defense-Fonds (Open Invention Network-Äquivalent in EU)

### 2.2 Export-Kontrolle

**Risiko:** Unser DDS wird Defense-Kunden bedienen. EU Dual-Use Regulation (EU 2021/821) kann Export-Lizenzen für bestimmte Kombinationen aus Crypto + Destination-Country erfordern.

**Impact:** Blocked exports, verzögerte Deals, Compliance-Overhead.

**Mitigation:**
- Phase-0-Legal-Beratung zu EU-Dual-Use-Klassifikation
- Separates Crypto-Plugin-Modell: Core-Distribution ohne Strong-Crypto, Crypto separat lizenzierbar je nach Destination
- Prüfen: BAFA-Klassifizierung in Deutschland, analog in anderen EU-Staaten
- Open-Source-Release-Strategie mit Blick auf US-EAR-ähnliche EU-Regelungen abstimmen

### 2.3 Open-Source-Lizenz-Wahl

**Risiko:** Falsche Lizenz-Wahl schließt entweder kommerzielle Kunden aus (z.B. GPL) oder gibt Konkurrenten Gratis-Code ohne Reziprozität.

**Impact:** Geschäftsmodell-Erosion oder Community-Adoption-Lücke.

**Empfehlung:**
- **Dual-Lizenz Apache 2.0 + MIT**: maximale Adoption, EU-freundlich, kommerziell friktionsfrei
- **Alternative: EPL 2.0**: wenn wir zu Eclipse Foundation gehen, was dort übliche Lizenz ist
- **Gegen GPL**: schließt embedded Vendor-Integration aus
- **Gegen AGPL**: schließt Cloud-SaaS-Deployments aus
- Commercial-Support + Services als primäres Business-Modell, nicht Lizenz-Verkauf

### 2.4 Trademark und Naming

**Risiko:** DDS-Markt hat bereits RTI Connext, eProsima Fast DDS, Cyclone DDS — einen neuen Namen zu finden, der unique ist und nicht mit existierenden Marken kollidiert, erfordert Recherche.

**Mitigation:**
- Phase-0-Trademark-Recherche (EUIPO, USPTO-Screening)
- Domain-Verfügbarkeit prüfen (`.eu`, `.org`, `.io`)
- GitHub-Organisation-Namen reservieren
- Keine DDS-Namen mit "Euro-" oder "Sovereign-" Präfix wählen (politisiert, schränkt Use-Cases ein)

## 3 Strategische und Wettbewerbs-Risiken

### 3.1 RTI-Reaktion

**Risiko:** RTI wird auf einen EU-Souveränitäts-Konkurrenten reagieren. Mögliche Reaktionen reichen von Akquisitions-Angeboten über aggressive Preis-Unterbietung bis zu rechtlichen Schritten.

**Impact:** Marktzugang erschwert, Investoren nervös, Team-Abwerbungen.

**Mitigation:**
- **Financial Runway:** mindestens 18 Monate Cash-Runway unabhängig von frühen Umsätzen
- **IP-Protection:** eigene Patent-Anmeldungen für Innovationen (Observability-Features, XRCE-Erweiterungen)
- **Community-Moat:** starke Open-Source-Community als Verteidigungslinie gegen Proprietärisierung
- **Sovereign-Kunden-Anchor:** frühzeitige Commitments von EU-Defense- oder Aerospace-Kunden als strategische Rückendeckung
- **Team-Retention:** Vesting-Strukturen, langfristige Anreize; Schlüssel-Engineers nicht ersetzbar

### 3.2 eProsima-Positionierung

**Risiko:** eProsima ist EU-basiert (Madrid), hat Safe DDS mit ASIL D, und bedient viele unserer Ziel-Kunden bereits. Sie sind der natürlichste Konkurrent in unserem Segment.

**Impact:** Kunden, die bereits eProsima nutzen, haben hohe Wechsel-Kosten.

**Mitigation:**
- Klare Differenzierung: wir bieten **umfassenderes Tooling, Rust-basierte Safety, PlatformIO-Integration** — Dinge, die eProsima nicht hat
- Migration-Tools und Co-Existence-Modus (unser Stack und eProsima im selben Netzwerk)
- Nicht auf "eProsima ersetzen" positionieren, sondern "nächste Generation von DDS" — weniger konfrontativ
- Evtl. Kooperations-Gespräche mit eProsima (gemeinsame Interop-Test-Bed, OMG-Plug-Fest-Koordination)

### 3.3 Markt-Timing

**Risiko:** In 22–27 Monaten kann der Markt sich verschoben haben. Software-Defined-Vehicle-Trend, Post-Quantum-Crypto-Pflicht, neue Standards — Timing-Risk ist real.

**Mitigation:**
- Phased Release (Beta ab Monat 14, v1.0 ab Monat 22, Cert ab Monat 27) mit Markt-Feedback-Integration
- Flexibles Feature-Scope: kritisch vs. nice-to-have identifizieren, bei Bedarf verschieben
- Kontinuierliche Market-Intelligence (Customer-Interviews, Conference-Attendance, Competitor-Watch)

### 3.4 Community-Aufbau

**Risiko:** Open-Source-Projekt ohne Community ist nur Code. Ohne externe Contributors, User-Feedback und externe Reviews wird das Projekt isoliert bleiben.

**Mitigation:**
- Public GitHub ab Phase 1 (auch wenn noch nicht produktionsreif)
- Transparente Roadmap und Design-Dokumente (dieser Satz)
- Conference-Präsenz: ROSCon, Embedded World, Eclipse Conference, OMG Meetings
- Governance-Modell das externe Contributors willkommen heißt (SIG-Struktur wie Kubernetes)
- Ernannter Community-Manager ab Phase 3
- Partnerschaften mit Universitäten (Forschungsprojekte, studentische Contributions)

### 3.5 Governance (entschieden)

**Entscheidung:** ZeroDDS ist ein internes Core-Projekt der Sponsor-Firma. Kein externes Governance-Framework, keine Foundation-Donation geplant. Lizenz ist **Apache 2.0**. Diese Wahl bewahrt Optionalität — ein späterer Wechsel zu einem offeneren Governance-Modell bleibt ohne Re-Licensing-Friktion möglich, falls strategische Gründe dafür entstehen. Umgekehrt wird keine Planung, Personalaufwand oder Aufmerksamkeit jetzt investiert.

Community-bezogene Fragen (Open-Source-Release, Contributor-License-Agreement, Donation-Ziel) sind Expansion-Era-Themen und werden in `06_roadmap.md` §8.1 Track E behandelt, falls sie relevant werden.

## 4 Team- und Organisations-Risiken

### 4.1 Senior-Talent-Akquisition

**Risiko:** DDS-Experten sind rar. RTPS-Expertise noch rarer. In einem kompetitiven Markt (insbesondere in Deutschland) ist die Rekrutierung herausfordernd.

**Mitigation:**
- Competitive Compensation inklusive Equity-Beteiligung
- Remote-First-Option in EU-Zeitzone, nicht auf Standort-Cluster beschränkt
- Conference-Präsenz und technische Publikationen zum Aufbau von Employer-Brand
- University-Partnerships für Junior-Talent-Pipeline
- Kein Requirement für Ex-DDS-Vendor-Erfahrung; Rust+Distributed-Systems-Experten können DDS lernen

### 4.2 Bus-Factor

**Risiko:** Wenige Personen mit tiefem Protocol-Verständnis. Ausfall einzelner Schlüssel-Engineers verursacht Projekt-Risiko.

**Mitigation:**
- Mindestens zwei Senior-Engineers pro Kern-Subsystem (Protocol, Platform, Quality)
- Dokumentations-Pflicht: Jede nicht-triviale Entscheidung im Code als ADR (Architecture Decision Record)
- Code-Review-Kultur: Jeder Senior reviewt regelmäßig Code von allen anderen, vermeidet Silos

### 4.3 Claude-Teams-Abhängigkeit

**Risiko:** Wenn das gesamte Entwicklungsmodell auf Claude-Augmentation basiert und Claude-Availability/Pricing/Capabilities sich ändern, entsteht Plattform-Lock-In.

**Mitigation:**
- Prompts und Workflows in Git versioniert, reproduzierbar
- Artefakte (generierter Code) sind portabel zu anderen LLM-Providern
- Kein Vendor-Lock-In auf Anthropic-spezifische Features jenseits von Standard-LLM-Capabilities
- Kontinuitäts-Plan: was wenn Claude morgen weg ist? Team muss ohne weiter arbeiten können, langsamer aber funktional.

## 5 Compliance- und Regulierungs-Risiken

### 5.1 Safety-Audit-Durchfall (Expansion-Era, Track C)

**Risiko:** Falls Track C aktiviert wird, könnte der formelle Audit Mängel finden, die größere Refactorings erfordern.

**Impact:** Zertifizierung verspätet sich, Produkt nicht für Safety-Segment nutzbar.

**Mitigation:**
- Safety-by-Architecture-Disziplin ab Tag 1 in Bootstrap-Era (siehe `04_safety_by_architecture.md`) reduziert Retrofit-Risiko erheblich
- Audit-Artefakte werden kontinuierlich aufgebaut, nicht erst bei Track-C-Aktivierung
- Safety-Engineer wird mit Track-C-Start Teil des Teams; interne Audit-Simulation vor externem Audit
- Puffer im Track-C-Zeitplan eingeplant (6–10 Monate Kalenderzeit inkl. Remediation)

### 5.2 Post-Quantum-Crypto-Pflicht

**Risiko:** EU regulatorische Vorgaben oder Kunden-Anforderungen könnten PQC-Support vor Phase-3-Release erzwingen.

**Mitigation:**
- Plugin-Architektur des `zerodds-security`-Crates ist PQC-ready
- Monitoring der NIST-PQC-Standardisierung und EU-Regulation
- Early-Implementation-Option: Kyber/Dilithium-Prototyp in Phase 2/3 als Feature-Preview
- Hybride Crypto-Suites (Classical + PQC) als Default bei Release

### 5.3 CRA (Cyber Resilience Act)

**Risiko:** EU Cyber Resilience Act (2024) stellt Anforderungen an Hersteller von Software mit digitalen Elementen, einschließlich Sicherheits-Updates und Vulnerability-Disclosure.

**Mitigation:**
- SBOM-Produktion per Release (CycloneDX)
- Vulnerability-Disclosure-Policy dokumentiert und implementiert (security.md, GPG-Schlüssel)
- CVE-Prozess: PSIRT-ähnliche Funktion im Team
- Kontinuierliches `cargo audit` in CI, Auto-Patch für kritische CVEs

## 6 Risiko-Register (zusammenfassend)

| ID | Risiko | Wahrscheinlichkeit | Impact | Era | Trend |
|---|---|---|---|---|---|
| T-01 | RTPS-Reliable-Bug | Hoch | Hoch | Bootstrap | stabil |
| T-02 | XTypes-Interop-Issues | Mittel | Mittel | Bootstrap | stabil |
| T-03 | Performance-Gap | Mittel | Mittel | Proof | sinkend mit Rust-Reife |
| T-04 | Ferrocene-Target-Lücke | Niedrig | Mittel | Expansion | sinkend |
| T-05 | Claude-Multiplikator-Enttäuschung | Mittel | Mittel | Bootstrap | sinkend mit Erfahrung |
| L-01 | RTI-Patent-Exposure | Mittel | Hoch | Expansion | stabil |
| L-02 | Export-Kontrolle | Niedrig | Mittel | Expansion | steigend mit Geopolitik |
| L-03 | Lizenz-Fehlwahl | — | — | entschieden | Apache 2.0 gewählt |
| S-01 | RTI-Reaktion | Niedrig → Hoch | Mittel | je nach Sichtbarkeit | steigend mit Traction |
| S-02 | eProsima-Wettbewerb | Hoch | Mittel | alle | stabil |
| S-03 | Markt-Timing | Mittel | Mittel | alle | stabil |
| S-04 | Community-Aufbau | — | Niedrig | Expansion Track E | konditional |
| S-05 | Governance-Home-Entscheidung | — | — | entschieden | interner Core gewählt |
| O-01 | Senior-Talent-Akquisition | Mittel | Hoch | alle | stabil |
| O-02 | Bus-Factor (bei 2–4 FTE erhöht) | Hoch | Hoch | Bootstrap | sinkend in Proof-Era |
| O-03 | Claude-Abhängigkeit | Mittel | Mittel | Bootstrap | stabil |
| O-04 | Scope-Creep durch kleines Team | Hoch | Hoch | Bootstrap | steuerbar mit Disziplin |
| O-05 | Interner Use-Case verschiebt sich | Mittel | Hoch | Bootstrap | projekt-spezifisch |
| C-01 | Safety-Audit-Durchfall | Niedrig | Hoch | Expansion Track C | sinkend mit Architektur-Disziplin |
| C-02 | PQC-Pflicht | Niedrig | Mittel | alle | steigend |
| C-03 | CRA-Compliance | Mittel | Mittel | Proof → Expansion | steigend |

Das Register wird monatlich in der Lead-Runde reviewt und aktualisiert.

**Neu in dieser Version:**
- O-04 (Scope-Creep): bei kleinem Team kritisch, Phase-Gate-Disziplin essenziell
- O-05 (Use-Case-Shift): wir sind unser erster Kunde, wenn der interne Use-Case sich ändert, verschiebt sich das Validierungs-Ziel

## 7 Strategische Entscheidungen: Status

### 7.1 Für Bootstrap-Era (entschieden)

- **Projekt-Name:** ZeroDDS (Trademark-Clearance in Expansion-Era, siehe §2.4)
- **Lizenz:** Apache 2.0
- **Governance:** internes Core-Projekt, keine Foundation
- **Team-Modell:** 2–4 interne Engineers + Claude-Teams-Augmentation
- **Ersten Kunde:** wir selbst (interne Zielanwendung als Validierungs-Anker)
- **Crypto-Default:** Standard-Suite aus DDS-Security 1.2 (AES-GCM, RSA/ECDSA) für Interop-Tag-Eins
- **Safety-Pfad:** Safety-by-Architecture ab Tag 1, aber Ferrocene-Integration und formaler Audit verschoben in Expansion-Era

### 7.2 Für Expansion-Era (aufgeschoben)

Die folgenden Entscheidungen werden erst getroffen, wenn Bootstrap-Proof erreicht ist und externe Mittel mobilisiert werden:

1. **Ferrous-Systems-Partnerschafts-Vertrag** — Scope und Konditionen (Track B)
2. **OMG-Membership-Tier** (Track A)
3. **Patent-Anwalt-Engagement** für Freedom-to-Operate und Trademark (Track D)
4. **EU-Crypto-Plugin-Strategie** — welche Suites als zweite Wahl nach der Standard-Suite
5. **Open-Source-Release-Strategie** (Track E) — falls Community-Build gewünscht
6. **HSM-Integration via PKCS#11** — als kommerzielles Feature oder Default?
7. **Governance-Evolution** — bleibt internes Core-Projekt oder Donation an Foundation?

Diese Aufschiebung ist bewusst. Mittel für externe Engagements werden rationaler allokiert, wenn ein funktionierender Stack als Verhandlungs-Basis existiert.
