# OMG-Standards

Detail-Eintraege zu allen OMG-Specs, auf denen ZeroDDS aufbaut. Kanonische Uebersicht und Verpflichtungs-Grade in [`INDEX.md`](./INDEX.md), Scope pro Phase in `docs/architecture/01_scope_and_specs.md`.

## Copyright-Hinweis

Alle hier referenzierten Specs sind Copyright **Object Management Group, Inc. (OMG)**. OMG stellt die Specs oeffentlich unter <https://www.omg.org/spec/> bereit. Diese Registry enthaelt ausschliesslich **Metadaten und Abbildungen auf ZeroDDS-Crates**, **keinen Text aus den Specs**. Spec-PDFs werden via `fetch.sh` lokal in `cache/omg/` geladen und sind git-ignored.

---

## 1 DDS DCPS — Data-Centric Publish-Subscribe

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-zerodds-dcps` |
| Offizieller Titel | Data Distribution Service (DDS) for Real-Time Systems |
| Version | 1.4 |
| Dokument-Nummer | formal/2015-04-10 |
| Veroeffentlicht | April 2015 |
| URL | <https://www.omg.org/spec/DDS/1.4/> |
| Cache-Pfad | `cache/omg/zerodds-dcps-1.4.pdf` |
| Lizenz | OMG Availability Statement (freier Download, kommerzielle Nutzung erlaubt) |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `zerodds-dcps`, `zerodds-qos` |

**Relevante Kapitel fuer ZeroDDS:**
- §2.2 Platform Independent Model — Entity-Modell
- §2.2.2 Domain Module — DomainParticipant, DomainParticipantFactory
- §2.2.3 Topic-Definition Module — Topic, TypeSupport, ContentFilteredTopic
- §2.2.4 Publication Module — Publisher, DataWriter, Listener
- §2.2.5 Subscription Module — Subscriber, DataReader, Listener, SampleInfo
- §2.2.6 Infrastructure Module — StatusCondition, WaitSet, GuardCondition
- §2.2.3 QoS Policies — alle 22 Standard-Policies, Request/Offered-Matching

**Conformance-Profile:**
- Minimum Profile — Phase 1 Ziel
- Ownership Profile — Phase 2
- Content-Subscription Profile — Phase 2
- Persistence Profile — Phase 3
- Object Model Profile — out of scope (DLRL-abhaengig)

---

## 2 DDSI-RTPS — Real-Time Publish-Subscribe Wire-Protokoll

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-ddsi-rtps` |
| Offizieller Titel | DDS Interoperability Wire Protocol Specification |
| Version | 2.5 |
| Dokument-Nummer | formal/2022-05-02 |
| Veroeffentlicht | Mai 2022 |
| URL | <https://www.omg.org/spec/DDSI-RTPS/2.5/> |
| Cache-Pfad | `cache/omg/ddsi-rtps-2.5.pdf` |
| Lizenz | OMG Availability Statement |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `zerodds-rtps`, `zerodds-discovery`, `zerodds-transport`, `zerodds-transport-udp`, `zerodds-transport-tcp`, `zerodds-transport-shm` |

**Relevante Kapitel:**
- §8.2 Structure Module — Entity-Modell, GUID, HistoryCache
- §8.3 Messages Module — Wire-Format, Submessages, Submessage-Elements
- §8.3.7 RTPS Submessages — Data, Heartbeat, AckNack, Gap, InfoTimestamp, InfoDestination, DataFrag, HeartbeatFrag, NackFrag
- §8.4 Behavior Module — Reader/Writer State Machines, Reliable-Protokoll
- §8.5 Discovery Module — SPDP, SEDP
- §8.7 Mapping to UDP/IPv4 — UDP-PSM
- Annex B (spec-vectors) — Test-Vektoren fuer Wire-Serialisierung (fuer Compliance-Tests)

**Nicht-implementiert:**
- Vendor-spezifische Wire-Extensions (RTI FlatData, OpenSplice DDSI2E) — explizit out-of-scope per `docs/architecture/01_scope_and_specs.md §5`

---

## 3 DDS-XTypes — Extensible and Dynamic Topic Types

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-dds-xtypes` |
| Offizieller Titel | Extensible and Dynamic Topic Types for DDS |
| Version | 1.3 |
| Dokument-Nummer | formal/2020-02-04 |
| Veroeffentlicht | Februar 2020 |
| URL | <https://www.omg.org/spec/DDS-XTypes/1.3/> |
| Cache-Pfad | `cache/omg/dds-xtypes-1.3.pdf` |
| Lizenz | OMG Availability Statement |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `zerodds-types`, `zerodds-cdr` |

**Relevante Kapitel:**
- §7.2 Type System — TypeKind, TypeObject, TypeIdentifier
- §7.3 Data Representation — XCDR1, XCDR2, Alignment, Endianness
- §7.4 Dynamic Language Binding — DynamicType, DynamicData
- §7.5 Annotations and Extensibility — @final, @appendable, @mutable, @optional, @key
- §7.6 Type Compatibility — Assignability-Regeln
- §7.7 Type Representation — XML, JSON, IDL
- §7.9 TypeLookup Service — Discovery von unbekannten Typen
- Annex — CompleteTypeObject Vektoren fuer Interop

**Wichtig fuer Interop:** @appendable-Default-Semantik weicht zwischen aelteren Vendor-Implementierungen ab. Wir richten uns strikt nach Spec 1.3 und dokumentieren Abweichungen.

---

## 4 DDS-Security

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-zerodds-security` |
| Offizieller Titel | DDS Security |
| Version | 1.2 |
| Dokument-Nummer | zerodds-security/1.2 |
| URL | <https://www.omg.org/spec/DDS-SECURITY/1.2/> |
| Cache-Pfad | `cache/omg/zerodds-security-1.2.pdf` |
| Lizenz | OMG Availability Statement |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `zerodds-security`, `zerodds-rtps` (secure-Submessages), `zerodds-dcps` (Permissions-Checks) |

**Relevante Kapitel:**
- §8 Plugin Architecture — Authentication, AccessControl, Cryptographic, Logging, DataTagging SPIs
- §9 Builtin Plugin — PKI-Authentication, AccessControl via Permissions-Document, AES-GCM/GMAC
- §7 Secure DDS Messages — RTPS-Submessages mit MAC und/oder Encryption
- Annex A — Permissions-Document-Schema
- Annex B — Governance-Document-Schema

**Plugin-Implementierungen in `zerodds-security`:**
- Standard-Suite (Default, Phase 2): AES-GCM, AES-GMAC, RSA/ECDSA, Ed25519
- Optional: PKCS#11-HSM-Integration (Expansion-Era)
- Optional: EU-Crypto-Suites (Expansion-Era, kundenabhaengig)
- Future: Post-Quantum-Crypto-Suite (NIST-Kyber/Dilithium, hybrid classical+PQC) — siehe `docs/architecture/07_risks_and_strategy.md §5.2`

---

## 5 IDL — Interface Definition Language

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-idl` |
| Offizieller Titel | Interface Definition Language |
| Version | 4.2 |
| Dokument-Nummer | formal/2018-01-05 |
| URL | <https://www.omg.org/spec/IDL/4.2/> |
| Cache-Pfad | `cache/omg/idl-4.2.pdf` |
| Lizenz | OMG Availability Statement |
| ISO-Equivalent | ISO/IEC 19516:2020 (paywalled, nicht gecached) |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `tools/idlc` (Compiler), alle Binding-Crates (Runtime-Support) |

**Relevante Kapitel:**
- §7.2 Building Blocks — Core, Anonymous-Types, Any, Interfaces, Template-Modules, Extended-Data-Types, Components, Home, Ports, CCM-Interfaces, IDL-Components, Value-Types, Annotations
- §7.3 IDL-Syntax — Grammar, Keywords, Lexical Elements
- §7.4 Type-System — alle Primitive-, Constructed-, Template-Typen
- §8 Standardized Annotations — @key, @nested, @autoid, @extensibility, @optional, @default, @range, @min, @max, @unit

**ZeroDDS-Building-Blocks:**
Wir implementieren die fuer DDS relevanten Building Blocks: Core, Anonymous-Types, Any (limitiert), Extended-Data-Types, Annotations, Value-Types. Components/Home/Ports sind CORBA-Erbe und fuer uns out-of-scope.

---

## 6 DDS-RPC — Remote Procedure Calls over DDS

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-zerodds-rpc` |
| Offizieller Titel | Remote Procedure Call over DDS |
| Version | 1.0 |
| Dokument-Nummer | formal/2017-04-01 |
| URL | <https://www.omg.org/spec/DDS-RPC/1.0/> |
| Cache-Pfad | `cache/omg/zerodds-rpc-1.0.pdf` |
| Lizenz | OMG Availability Statement |
| Verpflichtungs-Grad | conformance |
| ZeroDDS-Crates | `zerodds-rpc` |
| Phase | Phase 4 (siehe `06_roadmap.md §7` WP 4.4) |

**Relevante Kapitel:**
- §7 Service Definition — IDL-Interfaces zu DDS-Topics-Mapping
- §8 Request/Reply-Topic-Paare
- §9 Language Bindings — C++, Java

---

## 7 DDS-XML

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-zerodds-xml` |
| Offizieller Titel | DDS Consolidated XML Syntax |
| Version | 1.0 |
| URL | <https://www.omg.org/spec/DDS-XML/1.0/> |
| Cache-Pfad | `cache/omg/zerodds-xml-1.0.pdf` |
| Lizenz | OMG Availability Statement |
| Verpflichtungs-Grad | conformance |
| ZeroDDS-Crates | `zerodds-xml`, `tools/xmlc` |
| Phase | Phase 2 (siehe `06_roadmap.md §5` WP 2.7) |

**Scope:** XML-Darstellung fuer Types, QoS-Profile, Entity-Deployment-Descriptoren. XSD-Schemata werden in CI gegen DDS-XML-Beispiele validiert.

---

## 8 DDS-XRCE — eXtremely Resource-Constrained Environments

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-zerodds-xrce` |
| Offizieller Titel | DDS for eXtremely Resource Constrained Environments |
| Version | 1.0 |
| Dokument-Nummer | formal/2019-07-02 |
| URL | <https://www.omg.org/spec/DDS-XRCE/1.0/> |
| Cache-Pfad | `cache/omg/zerodds-xrce-1.0.pdf` |
| Lizenz | OMG Availability Statement |
| Verpflichtungs-Grad | conformance |
| ZeroDDS-Crates | `zerodds-xrce-client`, `zerodds-xrce-agent` |
| Phase | Phase 4 (siehe `06_roadmap.md §7` WP 4.5) |

**Relevante Kapitel:**
- §7 Client-Agent-Protokoll — Session-Lifecycle, Objekt-Model, Submessages
- §8 Transport-Mappings — UDP, TCP, Serial, CustomTransport
- §9 XRCE-to-DDS-Bridging — Topic-Mapping, QoS-Propagation

---

## 9 Language-Mappings

### 9.1 IDL4 to C++

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-idl4-cpp` |
| Offizieller Titel | IDL to C++ Language Mapping |
| Version | 1.3 |
| URL | <https://www.omg.org/spec/IDL4-CPP/> |
| Cache-Pfad | `cache/omg/idl4-cpp-1.3.pdf` |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `tools/idlc` (Backend), `zerodds-cpp` (Runtime) |

### 9.2 IDL4 to Java

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-idl4-java` |
| URL | <https://www.omg.org/spec/IDL4-Java/> |
| Cache-Pfad | `cache/omg/idl4-java-1.0.pdf` |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `tools/idlc`, `zerodds-java` |

### 9.3 IDL4 to C#

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-idl4-csharp` |
| URL | <https://www.omg.org/spec/IDL4-CSHARP/> |
| Cache-Pfad | `cache/omg/idl4-csharp-1.0.pdf` |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `tools/idlc`, `zerodds-cs` |

### 9.4 DDS C++ API PSM

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-dds-psm-cxx` |
| Offizieller Titel | DDS Platform Specific Model for ISO C++ |
| URL | <https://www.omg.org/spec/DDS-PSM-Cxx/> |
| Cache-Pfad | `cache/omg/dds-psm-cxx-1.0.pdf` |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `zerodds-cpp` |

### 9.5 DDS Java API PSM

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-zerodds-java-psm` |
| URL | <https://www.omg.org/spec/DDS-JAVA/> |
| Cache-Pfad | `cache/omg/zerodds-java-psm-1.0.pdf` |
| Verpflichtungs-Grad | normative |
| ZeroDDS-Crates | `zerodds-java` |

### 9.6 DDS C# API PSM

| Feld | Wert |
|---|---|
| Kurz-ID | `omg-dds-csharp-psm` |
| URL | <https://www.omg.org/spec/DDS-CSHARP/> |
| Verpflichtungs-Grad | conformance |
| ZeroDDS-Crates | `zerodds-cs` |

**Hinweis C#-PSM:** Die OMG-Spezifikation fuer das offizielle DDS C# PSM ist juenger und weniger breit in Vendor-Ecosystemen verbreitet als C++/Java. Wir orientieren uns an der Spec, legen aber besonderen Wert auf Interop mit den praktisch verbreiteten APIs (RTI Connext .NET, eProsima .NET).

---

## Nicht implementierte OMG-Specs (explizit out of scope)

Gemaess `docs/architecture/01_scope_and_specs.md §5`:

| Spec | Begruendung |
|---|---|
| DDS DLRL | Data-Local-Reconstruction-Layer, Spec-Teil verwaist |
| CORBA GIOP / IIOP | Historisches Erbe, keine Kunden-Nachfrage |
| DDS Object-Model Profile | DLRL-abhaengig |
| Real-time CORBA | CORBA-Erbe |
