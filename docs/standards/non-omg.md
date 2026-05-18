# Non-OMG-Standards

Detail-Eintraege zu allen nicht-OMG-Standards, auf denen ZeroDDS aufbaut. Kanonische Uebersicht und Verpflichtungs-Grade in [`INDEX.md`](./INDEX.md).

## Copyright-Hinweise nach Organisation

- **W3C:** W3C Document License, Redistribution mit Attribution zulaessig.
- **IETF:** RFCs stehen unter IETF Trust License (TLP), Redistribution weitgehend frei.
- **CNCF (OpenTelemetry):** Apache 2.0, Redistribution frei.
- **OASIS (PKCS#11):** OASIS IPR Policy, freier Download.
- **CycloneDX:** Apache 2.0.
- **SPDX:** Creative Commons CC-BY-3.0.
- **ISO/IEC:** kostenpflichtig, **nicht gecacht**, Bezug ueber Firmen-Lizenz.
- **RTCA/EUROCAE (DO-178C/ED-12C):** kostenpflichtig, nicht gecacht.

Wo rechtlich zulaessig und technisch sinnvoll, laedt `fetch.sh` die Dokumente in `cache/`.

---

## 1 W3C Trace Context

| Feld | Wert |
|---|---|
| Kurz-ID | `w3c-trace-context` |
| Titel | Trace Context Level 1 |
| Status | W3C Recommendation |
| URL | <https://www.w3.org/TR/trace-context/> |
| Cache-Pfad | `cache/w3c/trace-context.html` |
| Lizenz | W3C Document License |
| Verpflichtungs-Grad | integration |
| ZeroDDS-Crates | `zerodds-monitor`, `zerodds-rtps` |

**Verwendung:** Unsere PID_VENDOR_TRACE_CONTEXT-Parameter-List-Element-Definition (siehe `docs/architecture/05_observability_and_tooling.md §4.2`) transportiert `traceparent` und `tracestate` gemaess dieser Spec ueber RTPS. Level-2-CR wird beobachtet, aber nicht aktiv implementiert (siehe `w3c-trace-context-2` im Index).

**Wichtige Sections:**
- §3 `traceparent` Header Format
- §3.2 `tracestate` Header
- §3.3 Processing Model

---

## 2 IETF RFCs

### 2.1 Transport-Layer

| RFC | Titel | URL | Cache |
|---|---|---|---|
| RFC 768 | User Datagram Protocol | <https://datatracker.ietf.org/doc/html/rfc768> | `cache/ietf/rfc768.txt` |
| RFC 9293 | Transmission Control Protocol | <https://datatracker.ietf.org/doc/html/rfc9293> | `cache/ietf/rfc9293.txt` |
| RFC 1122 | Requirements for Internet Hosts — Communication Layers | <https://datatracker.ietf.org/doc/html/rfc1122> | `cache/ietf/rfc1122.txt` |

**Scope:** normativ fuer unsere Transport-Crates `zerodds-transport-udp`, `zerodds-transport-tcp`. RTPS nutzt UDP/IPv4 als Default-Transport.

### 2.2 Identitaet und Zeit

| RFC | Titel | Verpflichtung |
|---|---|---|
| RFC 4122 | UUID URN Namespace | reference (GUID-Darstellung) |
| RFC 5905 | NTPv4 | reference (Wall-Clock-Synchronisation, relevant fuer Wire-Recorder §6.2) |

### 2.3 Security

| RFC | Titel | Verpflichtung |
|---|---|---|
| RFC 8446 | TLS 1.3 | integration (TLS-gesicherte Discovery-Transport-Variante) |
| RFC 5280 | Internet X.509 PKI Certificate und CRL Profile | normative (fuer DDS-Security PKI-Auth) |
| RFC 8032 | EdDSA (Ed25519) | normative (Signatur-Algorithmus in `zerodds-security`) |
| RFC 5116 | AEAD Authenticated Encryption | normative (AES-GCM Einbettung in DDS-Security Crypto-Plugin) |

### 2.4 HTTP und CBOR (fuer OTel-Export)

| RFC | Titel | Verpflichtung |
|---|---|---|
| RFC 9110 | HTTP Semantics | integration (OTLP/HTTP) |
| RFC 9112 | HTTP/1.1 | integration |
| RFC 9113 | HTTP/2 | integration |
| RFC 8949 | CBOR | reference |
| RFC 7049 | CBOR (obsoleted by 8949) | — |

---

## 3 CNCF / Observability

### 3.1 OpenTelemetry Specification

| Feld | Wert |
|---|---|
| Kurz-ID | `otel-spec` |
| Titel | OpenTelemetry Specification |
| Version | aktueller Stable-Stand (wird pro ZeroDDS-Release gepinnt) |
| Repository | <https://github.com/open-telemetry/opentelemetry-specification> |
| Cache-Pfad | `cache/otel/specification/` (Git-Mirror) |
| Lizenz | Apache 2.0 |
| Verpflichtungs-Grad | integration |
| ZeroDDS-Crates | `zerodds-monitor` |

**Relevante Teile:**
- `specification/overview.md` — Datenmodell
- `specification/trace/api.md` — Span/Tracer API
- `specification/metrics/api.md` — Metrics API
- `specification/logs/api.md` — Logs API
- `specification/context/` — Context Propagation mit W3C Trace Context
- `semantic_conventions/` — `messaging.*` und `network.*` als Basis, unsere `dds.*`-Attribute ergaenzend (siehe `docs/architecture/05_observability_and_tooling.md §4.1`)

### 3.2 OpenTelemetry Protocol (OTLP)

| Feld | Wert |
|---|---|
| Kurz-ID | `otlp` |
| Repository | <https://github.com/open-telemetry/opentelemetry-proto> |
| Cache-Pfad | `cache/otel/proto/` |
| Lizenz | Apache 2.0 |
| Verpflichtungs-Grad | integration |

**Scope:** wir exportieren Traces, Metrics, Logs via OTLP/gRPC und OTLP/HTTP. Keine proprietaere Erweiterungen.

### 3.3 OpenMetrics

| Feld | Wert |
|---|---|
| Kurz-ID | `openmetrics` |
| Titel | OpenMetrics — A Cloud-Native, Highly Scalable Metrics Protocol |
| Version | 1.0 |
| URL | <https://github.com/OpenObservability/OpenMetrics/blob/main/specification/OpenMetrics.md> |
| Cache-Pfad | `cache/openmetrics/openmetrics-1.0.md` |
| Lizenz | Apache 2.0 |
| Verpflichtungs-Grad | integration |

### 3.4 Prometheus Exposition Format

| Feld | Wert |
|---|---|
| Kurz-ID | `prometheus-textformat` |
| URL | <https://prometheus.io/docs/instrumenting/exposition_formats/> |
| Cache-Pfad | `cache/prometheus/exposition-formats.html` |
| Lizenz | Apache 2.0 (Prometheus Docs) |
| Verpflichtungs-Grad | integration |

**Scope:** `zerodds-monitor` exportiert Metriken im Prometheus-Textformat **und** als OTLP. Siehe `docs/architecture/05_observability_and_tooling.md §3` fuer den Metrik-Katalog.

---

## 4 OASIS

### 4.1 PKCS#11 Cryptographic Token Interface

| Feld | Wert |
|---|---|
| Kurz-ID | `pkcs11` |
| Titel | PKCS #11 Cryptographic Token Interface Base Specification |
| Version | 3.1 |
| URL | <https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.1/pkcs11-base-v3.1.html> |
| Cache-Pfad | `cache/oasis/pkcs11-3.1.html` |
| Lizenz | OASIS IPR Policy |
| Verpflichtungs-Grad | future (Expansion-Era) |
| ZeroDDS-Crates | `zerodds-security` (HSM-Plugin) |

**Scope:** optionales Feature in `zerodds-security` fuer Hardware-Security-Module-Integration. Konkrete Implementierung in Expansion-Era, siehe `docs/architecture/07_risks_and_strategy.md §7.2`.

---

## 5 Sonstige

### 5.1 CycloneDX SBOM

| Feld | Wert |
|---|---|
| Kurz-ID | `cyclonedx` |
| Titel | CycloneDX Bill of Materials Standard |
| Version | 1.6 |
| URL | <https://cyclonedx.org/specification/overview/> |
| Cache-Pfad | `cache/cyclonedx/spec-1.6.html` |
| Lizenz | Apache 2.0 |
| Verpflichtungs-Grad | integration |

**Scope:** pro Release generiert CI ein CycloneDX-SBOM, siehe `docs/architecture/01_scope_and_specs.md §2`. CRA-relevant (`docs/architecture/07_risks_and_strategy.md §5.3`).

### 5.2 SPDX License List

| Feld | Wert |
|---|---|
| Kurz-ID | `spdx` |
| URL | <https://spdx.org/licenses/> |
| Lizenz | CC-BY-3.0 |
| Verpflichtungs-Grad | integration |

**Scope:** wir deklarieren Lizenzen in Cargo.toml und SBOM strikt via SPDX-Identifier. Primaere Projekt-Lizenz: `Apache-2.0` (zukuenftig optional `Apache-2.0 OR MIT`, siehe `docs/architecture/02_architecture.md §5`).

### 5.3 Semantic Versioning

| Feld | Wert |
|---|---|
| Kurz-ID | `semver` |
| Version | 2.0.0 |
| URL | <https://semver.org/spec/v2.0.0.html> |
| Lizenz | Creative Commons |
| Verpflichtungs-Grad | normative |

**Scope:** Versionierungs-Politik aus `docs/architecture/01_scope_and_specs.md §6`, mit Tier-basierter Strenge aus `02_architecture.md §7`.

### 5.4 Conventional Commits

| Feld | Wert |
|---|---|
| Kurz-ID | `conventional-commits` |
| Version | 1.0.0 |
| URL | <https://www.conventionalcommits.org/en/v1.0.0/> |
| Lizenz | MIT |
| Verpflichtungs-Grad | normative |

**Scope:** Commit-Message-Konvention mit `[REQ-...]`-Requirements-Tag siehe `docs/architecture/04_safety_by_architecture.md §4.1`.

### 5.5 PlatformIO Library Manifest

| Feld | Wert |
|---|---|
| Kurz-ID | `platformio-manifest` |
| URL | <https://docs.platformio.org/en/latest/manifests/library-json/> |
| Lizenz | Apache 2.0 (PlatformIO) |
| Verpflichtungs-Grad | integration |
| ZeroDDS-Artefakte | Micro-Profile Release-Package (`library.json`) |

**Scope:** siehe `docs/architecture/03_profiles_and_platforms.md §6`.

---

## 6 ISO / IEC / DO — Safety-Standards (paywalled)

Fuer die formale Safety-Zertifizierung (Expansion-Era, Track C) relevant. **Nicht im Cache**, Bezug ueber Firmen-Abonnement oder Erwerb bei DIN / Beuth / SAE / RTCA.

### 6.1 ISO 26262 — Road Vehicles Functional Safety

| Teil | Titel |
|---|---|
| ISO 26262-1:2018 | Vocabulary |
| ISO 26262-2:2018 | Management of functional safety |
| ISO 26262-3:2018 | Concept phase |
| ISO 26262-4:2018 | Product development at the system level |
| ISO 26262-5:2018 | Product development at the hardware level |
| ISO 26262-6:2018 | Product development at the software level — **primaer relevant** |
| ISO 26262-7:2018 | Production, operation, service and decommissioning |
| ISO 26262-8:2018 | Supporting processes |
| ISO 26262-9:2018 | Automotive safety integrity level (ASIL)-oriented and safety-oriented analyses |
| ISO 26262-10:2018 | Guidelines on ISO 26262 |
| ISO 26262-11:2018 | Guidelines on application of ISO 26262 to semiconductors |
| ISO 26262-12:2018 | Adaptation of ISO 26262 for motorcycles |

**ZeroDDS-Relevanz:** ASIL D fuer Safe-Subset, siehe `docs/architecture/04_safety_by_architecture.md §6.4`.

### 6.2 IEC 61508 — Functional Safety of E/E/PE Systems

| Teil | Titel |
|---|---|
| IEC 61508-1 | General requirements |
| IEC 61508-2 | Requirements for electrical/electronic/programmable electronic safety-related systems |
| IEC 61508-3 | Software requirements — **primaer relevant** |
| IEC 61508-4 | Definitions and abbreviations |
| IEC 61508-5 | Examples of methods for the determination of safety integrity levels |
| IEC 61508-6 | Guidelines on the application of IEC 61508-2 and IEC 61508-3 |
| IEC 61508-7 | Overview of techniques and measures |

**ZeroDDS-Relevanz:** SIL 3 fuer Safe-Subset.

### 6.3 IEC 62304 — Medical Device Software

| Titel | Relevanz |
|---|---|
| IEC 62304:2006 + A1:2015 | Medical device software — Software life cycle processes |

**ZeroDDS-Relevanz:** Class C (sekundaer, abhaengig von Kunden-Nachfrage).

### 6.4 DO-178C / ED-12C — Avionics Software

| Dokument | Titel |
|---|---|
| RTCA DO-178C / EUROCAE ED-12C | Software Considerations in Airborne Systems and Equipment Certification |
| RTCA DO-330 / ED-215 | Software Tool Qualification Considerations — **fuer Ferrocene-Qualifikation relevant** |
| RTCA DO-331 / ED-218 | Model-Based Development and Verification Supplement |
| RTCA DO-332 / ED-217 | Object-Oriented Technology and Related Techniques Supplement |
| RTCA DO-333 / ED-216 | Formal Methods Supplement — **relevant fuer Kani-Model-Checking-Argumentation** |

**ZeroDDS-Relevanz:** DAL B initial, DAL A perspektivisch.

### 6.5 EN 50128 / 50716 — Railway

| Titel | Status |
|---|---|
| EN 50128:2011 + A2:2020 | Railway applications — Communication, signalling and processing systems — Software for railway control and protection systems |
| EN 50716:2023 | Nachfolger-Norm (konsolidiert EN 50128 + EN 50129) |

**ZeroDDS-Relevanz:** sekundaer, fuer Bahn-Signaltechnik-Kunden.

### 6.6 ISO/IEC 19516:2020 — IDL

| Titel | Status |
|---|---|
| ISO/IEC 19516:2020 | Information technology — Object management group interface definition language (IDL) |

**ZeroDDS-Relevanz:** reference. Die OMG-IDL-4.2-Spec ist inhaltsgleich und fuer uns kanonisch (siehe [`omg.md §5`](./omg.md#5-idl--interface-definition-language)).

---

## 7 Werkzeuge und Toolchains

Kein Standard im engeren Sinne, aber versions-kritisch und damit Teil der Registry.

### 7.1 Rust Toolchain

- Pinned in `rust-toolchain.toml` im Repo-Root.
- Aktueller Pin: **stable 1.85** (Bootstrap-/Proof-Era).
- Policy: Toolchain-Upgrades nur per ADR (Bus-Factor-Schutz, CI-Breakage-Vermeidung).
- URL: <https://www.rust-lang.org/>

### 7.2 Ferrocene

- Commercial qualified Rust compiler by Ferrous Systems.
- Zertifiziert: ISO 26262 ASIL D, IEC 61508 SIL 3, IEC 62304 Class C (aktueller Stand pruefen beim Track-B-Start).
- Aktivierung: **Expansion-Era, Track B** (siehe `docs/architecture/06_roadmap.md §8.1`).
- URL: <https://ferrocene.dev/>
