# Scope und Spezifikations-Coverage

> **Status:** Draft v0.2
> **Abhängigkeiten:** `00_overview.md`

## 1 OMG-Spec-Family im Scope

Die folgenden OMG-Spezifikationen werden vollständig implementiert. Versionen spiegeln den Stand der jeweils aktuellen formalen Spec wider; neuere Revisionen werden nach Release geprüft und ggf. adoptiert.

### 1.1 Kern-Specs (Pflicht)

| Spec | Version | Scope | Implementierung |
|---|---|---|---|
| **DDS DCPS** | 1.4 (formal/2015-04-10) | Data-Centric Publish-Subscribe API, Entity-Modell, QoS-System | `zerodds-dcps`, `zerodds-qos` |
| **DDSI-RTPS** | 2.5 | Wire-Protokoll, Reader/Writer/Heartbeat-Semantik, Fragmentation | `zerodds-rtps` |
| **DDS-XTypes** | 1.3 (formal/2020-02-04) | Type System, TypeObject, XCDR1/XCDR2, @appendable/@mutable | `zerodds-types`, `zerodds-cdr` |
| **DDS-Security** | 1.2 | Authentication, Access Control, Cryptographic SPI | `zerodds-security` |
| **IDL4** | 4.2 (ISO/IEC 19516:2020) | Interface Definition Language, Building Blocks | `zerodds-idl`, `zerodds-idlc` |

### 1.2 Erweiterte Specs (Pflicht für Gesamt-Scope)

| Spec | Version | Scope | Implementierung |
|---|---|---|---|
| **DDS-RPC** | 1.0 | Request/Reply über DDS, Service-Definition | `zerodds-rpc` |
| **DDS-XML** | 1.0 | XML-Syntax für DDS-Resourcen, QoS-Profiles, Deployment | `zerodds-xml` |
| **DDSI-RTPS TCP/IP PSM** | (mit RTPS 2.5) | TCP-Mapping für NAT/Firewall-Szenarien | `zerodds-transport-tcp` |
| **DDS-XRCE** | 1.0 | Extremely Resource-Constrained Environments: Client-Agent-Protokoll | `zerodds-xrce-client`, `zerodds-xrce-agent` |

### 1.3 Language Mappings

| Spec | Target | Implementierung |
|---|---|---|
| **IDL4-C++** | C++17+ Mapping | Backend in `zerodds-idlc`, Runtime in `zerodds-cpp` |
| **IDL4-Java** | Java 11+ Mapping | Backend in `zerodds-idlc`, Runtime in `zerodds-java` |
| **IDL4-C#** | .NET 8+ Mapping | Backend in `zerodds-idlc`, Runtime in `zerodds-cs` |
| DDS C++ API PSM (ISO/IEC C++) | C++-API für DCPS | `zerodds-cpp` |
| DDS Java API PSM | Java-API für DCPS | `zerodds-java` |
| **DDS C#-API (ZeroDDS-eigener Anteil, nicht OMG-normiert)** | C#-API für DCPS | `zerodds-cs` |

**Hinweis zu DDS C#:** Die OMG hat kein formales C#-PSM fuer DCPS standardisiert.
Lediglich das IDL4-to-C#-Mapping (OMG `IDL4-CSHARP`) ist OMG-normiert und bildet
die Basis fuer Datentyp-Codegenerierung. Die Laufzeit-API `zerodds-cs` ist ein
**ZeroDDS-eigener, nicht OMG-normierter Anteil**. Sie folgt idiomatischen
.NET-Konventionen und orientiert sich an den Strukturen des C++-PSM (DDS-PSM-Cxx)
und der Rust-API (`zerodds-rs`), um Konsistenz ueber Bindings hinweg zu wahren.
Die API ist unter **Tier 1 Stabilitaet** gefuehrt (siehe `02_architecture.md §7`),
obwohl sie keine OMG-Spec-Pflicht hat — Stabilitaet gilt gegenueber unseren
Nutzern, nicht gegenueber der OMG-Community.

## 2 Non-OMG-Standards und externe Integration

| Standard | Zweck | Integration |
|---|---|---|
| **W3C Trace Context** | Distributed-Tracing-Header-Propagation | Als RTPS-Parameter-List-Element in jedem Sample (optional) |
| **OpenTelemetry** | Metrics, Traces, Logs Emission | Native Instrumentation in `zerodds-monitor`, OTLP-Export |
| **Prometheus Text-Format** | Metrics-Scraping | Exporter in `zerodds-monitor` |
| **OpenSSL / BoringSSL / Rust Crypto** | Kryptographische Primitive für DDS-Security | Plugin-basiert, swapbar für EU-Crypto-Suites |
| **PKCS#11** | HSM-Integration für Authentication | Optional-Feature in `zerodds-security` |
| **PlatformIO Library Registry** | Embedded-Distribution | Build-Pipeline in CI, `library.json` pro Release |
| **CycloneDX SBOM** | Software Bill of Materials | Pro-Release-Artefakt, CI-generiert |

## 3 Conformance-Profile-Ziele

DDS DCPS 1.4 definiert fünf Conformance-Profile. Unsere Ziele nach Phase:

| Profile | Phase 1 | Phase 2 | Phase 3 | Phase 4 |
|---|---|---|---|---|
| **Minimum Profile** | ✓ | ✓ | ✓ | ✓ |
| **Ownership Profile** | — | ✓ | ✓ | ✓ |
| **Content-Subscription Profile** | — | ✓ | ✓ | ✓ |
| **Persistence Profile** | — | — | ✓ | ✓ |
| **Object Model Profile** | — | — | — | (evaluieren; geringe Marktrelevanz) |

XTypes-Conformance: wir zielen auf **Complete** (Basic + Dynamic) bis Ende Phase 2.

RTPS-Interoperability: **Basic Profile** ab Ende Phase 1, **Minimal Profile** im Discovery-Bereich ab Ende Phase 1.

## 4 Interoperability-Ziele

Wire-Interop wird gegen die folgenden Peers kontinuierlich in CI und regelmäßig am OMG Plug-Fest validiert:

| Peer | Ziel-Version | Priorität |
|---|---|---|
| Eclipse Cyclone DDS | aktuell + LTS | Hoch — Default-ROS-2-RMW |
| eProsima Fast DDS | aktuell + LTS | Hoch — Default-ROS-2-RMW ab Humble |
| RTI Connext | 7.x LTS | Hoch — Marktführer, Defense-Relevanz |
| OCI OpenDDS | aktuell | Mittel |
| TwinOaks CoreDX | aktuell | Niedrig — kleinere Nutzerbasis |

Unsere OMG-Vendor-ID wird in Phase 0 beantragt. Vor Erhalt verwenden wir temporär die Vendor-ID-Range für Tests (zweistellige Entwickler-IDs).

## 5 Explizit außerhalb des Scopes

Folgende OMG-Specs und artverwandte Standards werden bewusst **nicht** implementiert:

| Excluded | Begründung |
|---|---|
| DDS DLRL (Data Local Reconstruction Layer) | Weitgehend verwaister Spec-Teil, keine relevanten Deployments |
| CORBA / GIOP Interop | Historisches Erbe, keine Kunden-Nachfrage |
| RTI FlatData, OpenSplice DDSI2E, Vendor-spezifische Extensions | Souveränitäts-Prinzip: wir bleiben bei Standard-Wire |
| JMS-Bridge | Kein strategisches Ziel |
| Web Integration (DDS-WebSocket, WebDDS-Prototypes) | Niedrige Priorität, evaluieren in späterer Phase |

## 6 Versionierungs-Politik

- Unser Produkt folgt **SemVer** auf allen Public-API-Surfaces.
- OMG-Spec-Versions-Pinning pro Release-Notes dokumentiert.
- Wire-Kompatibilität: **RTPS 2.5** ist Minimum. Downgrade auf 2.3/2.4 wird über Discovery-Negotiation unterstützt wo möglich.
- **Safe-Profile-API-Stabilität:** Ab Version 1.0 werden API-Änderungen in Safe-Subset-Crates durch formelles Change-Request-Verfahren behandelt, um Re-Zertifizierungs-Kosten zu minimieren.

## 7 Spec-to-Code-Traceability

Jede OMG-Spec-Section wird auf konkrete Code-Module abgebildet. Die Traceability-Matrix wird kontinuierlich gepflegt (Claude-Teams-unterstützt aus Git-History und Code-Annotationen).

Format der Annotation im Code:

```rust
/// Implements DDS-RTPS 2.5 §8.3.7.3 Heartbeat Submessage
#[spec(rtps = "2.5", section = "8.3.7.3")]
pub struct Heartbeat { ... }
```

Diese Annotationen werden vom Tool `zerodds-traceability` aggregiert in eine Coverage-Matrix, die sowohl für interne Reviews als auch für Safety-Audits als Grundlage dient.
