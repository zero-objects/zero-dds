# Standards-Index

Kompakte Gesamt-Uebersicht aller externen Standards, auf denen ZeroDDS aufbaut. Details pro Standard in [`omg.md`](./omg.md) und [`non-omg.md`](./non-omg.md). Legende zum Verpflichtungs-Grad in [`README.md`](./README.md).

## OMG — Data Distribution Service

| Kurz-ID | Spec | Version | Dokument-Nr. | Verpflichtung | Relevante Crates |
|---|---|---|---|---|---|
| `omg-zerodds-dcps` | DDS DCPS | 1.4 | formal/2015-04-10 | normative | `zerodds-dcps`, `zerodds-qos` |
| `omg-ddsi-rtps` | DDSI-RTPS Wire-Protokoll | 2.5 | formal/2022-05-02 | normative | `zerodds-rtps`, `zerodds-transport-*` |
| `omg-dds-xtypes` | DDS-XTypes | 1.3 | formal/2020-02-04 | normative | `zerodds-types`, `zerodds-cdr` |
| `omg-zerodds-security` | DDS-Security | 1.2 | zerodds-security/1.2 | normative | `zerodds-security` |
| `omg-idl` | IDL | 4.2 | formal/2018-01-05 | normative | `zerodds-idlc`, alle Binding-Crates |
| `omg-zerodds-rpc` | DDS-RPC | 1.0 | formal/2017-04-01 | conformance | `zerodds-rpc` |
| `omg-zerodds-xml` | DDS-XML | 1.0 | formal/2024 | conformance | `zerodds-xml`, `tools/xmlc` |
| `omg-zerodds-xrce` | DDS-XRCE | 1.0 | formal/2019-07-02 | conformance | `zerodds-xrce-client`, `zerodds-xrce-agent` |

### Language-Mappings

| Kurz-ID | Spec | Version | Verpflichtung | Relevante Crates |
|---|---|---|---|---|
| `omg-idl4-cpp` | IDL4 to C++ Mapping | 1.3 | normative | `zerodds-idlc`, `zerodds-cpp` |
| `omg-idl4-java` | IDL4 to Java Mapping | 1.0 | normative | `zerodds-idlc`, `zerodds-java` |
| `omg-idl4-csharp` | IDL4 to C# Mapping | 1.0 | normative | `zerodds-idlc`, `zerodds-cs` |
| `omg-dds-psm-cxx` | DDS C++ API PSM | 1.0 | normative | `zerodds-cpp` |
| `omg-zerodds-java-psm` | DDS Java API PSM | 1.0 | normative | `zerodds-java` |
| `omg-dds-csharp-psm` | DDS C# API PSM | 1.0 | conformance | `zerodds-cs` |

## W3C

| Kurz-ID | Spec | Version | Verpflichtung | Relevante Crates |
|---|---|---|---|---|
| `w3c-trace-context` | Trace Context | Level 1 (REC) | integration | `zerodds-monitor`, `zerodds-rtps` (PID_VENDOR_TRACE_CONTEXT) |
| `w3c-trace-context-2` | Trace Context | Level 2 (CR) | future | siehe oben |

## IETF — RFCs

| Kurz-ID | RFC | Titel | Verpflichtung | Relevante Crates |
|---|---|---|---|---|
| `rfc-768` | RFC 768 | User Datagram Protocol | normative | `zerodds-transport-udp` |
| `rfc-9293` | RFC 9293 | Transmission Control Protocol | normative | `zerodds-transport-tcp` |
| `rfc-4122` | RFC 4122 | UUID URN Namespace | reference | `zerodds-foundation` (GUID-Formatierung) |
| `rfc-8446` | RFC 8446 | TLS 1.3 | integration | `zerodds-security` (optionale TLS-gesicherte Discovery) |
| `rfc-8949` | RFC 8949 | CBOR | reference | Evaluation fuer Recording-Metadaten |
| `rfc-9110` | RFC 9110 | HTTP Semantics | integration | OTLP/HTTP-Exporter in `zerodds-monitor` |

## CNCF / Observability

| Kurz-ID | Spec | Version | Verpflichtung | Relevante Crates |
|---|---|---|---|---|
| `otel-spec` | OpenTelemetry Specification | v1.x (current) | integration | `zerodds-monitor` |
| `otlp` | OpenTelemetry Protocol | v1.x | integration | `zerodds-monitor` |
| `openmetrics` | OpenMetrics | 1.0 | integration | `zerodds-monitor` (Prometheus-Exporter) |
| `prometheus-textformat` | Prometheus Exposition Format | 0.0.4 | integration | `zerodds-monitor` |

## OASIS / Security

| Kurz-ID | Spec | Version | Verpflichtung | Relevante Crates |
|---|---|---|---|---|
| `pkcs11` | PKCS#11 Cryptographic Token Interface | 3.1 | future | `zerodds-security` (HSM-Plugin, Expansion-Era) |

## Sonstige

| Kurz-ID | Spec | Version | Verpflichtung | Relevante Crates / Artefakte |
|---|---|---|---|---|
| `cyclonedx` | CycloneDX SBOM | 1.6 | integration | CI-Release-Pipeline |
| `spdx` | SPDX License Identifier List | aktuell | integration | License-Deklaration pro Crate |
| `semver` | Semantic Versioning | 2.0.0 | normative | Alle Releases |
| `conventional-commits` | Conventional Commits | 1.0.0 | normative | Git-Commits gemaess `docs/architecture/04_safety_by_architecture.md §4.1` |
| `platformio-manifest` | PlatformIO Library Manifest | aktuell | integration | Embedded-Distribution `library.json` |

## ISO / IEC — paywalled, nicht im Cache

Fuer Safety-Zertifizierung relevante Standards, die kostenpflichtig bei ISO/DIN zu beziehen sind. Nur die Metadaten stehen hier; Bezug liegt beim Safety-Engineer / ueber Firmen-Abonnement.

| Kurz-ID | Norm | Titel | Era | Relevant fuer |
|---|---|---|---|---|
| `iso-26262` | ISO 26262 (alle Teile) | Road vehicles — Functional safety | Expansion Track C | Safe-Profile-Audit |
| `iec-61508` | IEC 61508 (alle Teile) | Functional safety of E/E/PE systems | Expansion Track C | Safe-Profile-Audit |
| `iec-62304` | IEC 62304 | Medical device software | Expansion Track C | Safe-Profile-Audit |
| `do-178c` | DO-178C / ED-12C | Software Considerations in Airborne Systems | Expansion Track C | Safe-Profile-Audit |
| `en-50128` | EN 50128 / 50716 | Railway applications — SW for railway control | Expansion Track C (sekundaer) | Safe-Profile-Audit |
| `iso-19516` | ISO/IEC 19516:2020 | IDL — Interface Definition Language | reference | Alternative zur OMG-IDL-Spec; OMG-Version ist fuer uns kanonisch |

**Hinweis Safety-Standards:** alle ISO-/IEC-/DO-Standards sind **Expansion-Era-relevant** (siehe `docs/architecture/06_roadmap.md §8.1 Track C`). In Bootstrap- und Proof-Era halten wir uns an die Architektur-Disziplin aus `docs/architecture/04_safety_by_architecture.md`, konsumieren die Standards aber nicht formell.

## Werkzeuge und Toolchains (kein Standard, aber Versions-kritisch)

| Kurz-ID | Werkzeug | Gepinnt in | Verpflichtung |
|---|---|---|---|
| `rust-toolchain` | stable Rust | `rust-toolchain.toml` | normative |
| `ferrocene` | Ferrocene qualified Rust | Expansion-Era (Track B) | future |

## Legend / Stand

Dieser Index spiegelt den Stand von ZeroDDS **Draft v0.2**. Aenderungen an Versionen erfolgen per ADR (siehe [`README.md`](./README.md) §Updates).
