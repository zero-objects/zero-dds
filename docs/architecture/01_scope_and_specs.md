# Scope and specification coverage

> **Status:** Draft v0.2
> **Dependencies:** `00_overview.md`

## 1 OMG spec family in scope

The following OMG specifications are implemented completely. Versions reflect the state of the respective current formal spec; newer revisions are reviewed after release and adopted if applicable.

### 1.1 Core specs (mandatory)

| Spec | Version | Scope | Implementation |
|---|---|---|---|
| **DDS DCPS** | 1.4 (formal/2015-04-10) | Data-Centric Publish-Subscribe API, entity model, QoS system | `zerodds-dcps`, `zerodds-qos` |
| **DDSI-RTPS** | 2.5 | Wire protocol, reader/writer/heartbeat semantics, fragmentation | `zerodds-rtps` |
| **DDS-XTypes** | 1.3 (formal/2020-02-04) | Type system, TypeObject, XCDR1/XCDR2, @appendable/@mutable | `zerodds-types`, `zerodds-cdr` |
| **DDS-Security** | 1.2 | Authentication, Access Control, Cryptographic SPI | `zerodds-security` |
| **IDL4** | 4.2 (ISO/IEC 19516:2020) | Interface Definition Language, building blocks | `zerodds-idl`, `zerodds-idlc` |

### 1.2 Extended specs (mandatory for full scope)

| Spec | Version | Scope | Implementation |
|---|---|---|---|
| **DDS-RPC** | 1.0 | Request/reply over DDS, service definition | `zerodds-rpc` |
| **DDS-XML** | 1.0 | XML syntax for DDS resources, QoS profiles, deployment | `zerodds-xml` |
| **DDSI-RTPS TCP/IP PSM** | (with RTPS 2.5) | TCP mapping for NAT/firewall scenarios | `zerodds-transport-tcp` |
| **DDS-XRCE** | 1.0 | Extremely Resource-Constrained Environments: client-agent protocol | `zerodds-xrce-client`, `zerodds-xrce-agent` |

### 1.3 Language mappings

| Spec | Target | Implementation |
|---|---|---|
| **IDL4-C++** | C++17+ mapping | Backend in `zerodds-idlc`, runtime in `zerodds-cpp` |
| **IDL4-Java** | Java 11+ mapping | Backend in `zerodds-idlc`, runtime in `zerodds-java` |
| **IDL4-C#** | .NET 8+ mapping | Backend in `zerodds-idlc`, runtime in `zerodds-cs` |
| DDS C++ API PSM (ISO/IEC C++) | C++ API for DCPS | `zerodds-cpp` |
| DDS Java API PSM | Java API for DCPS | `zerodds-java` |
| **DDS C# API (ZeroDDS-own part, not OMG-standardized)** | C# API for DCPS | `zerodds-cs` |

**Note on DDS C#:** The OMG has not standardized a formal C# PSM for DCPS.
Only the IDL4-to-C# mapping (OMG `IDL4-CSHARP`) is OMG-standardized and forms
the basis for data-type code generation. The runtime API `zerodds-cs` is a
**ZeroDDS-own part, not OMG-standardized**. It follows idiomatic
.NET conventions and orients itself on the structures of the C++ PSM (DDS-PSM-Cxx)
and the Rust API (`zerodds-rs`), to maintain consistency across bindings.
The API is tracked at **Tier 1 stability** (see `02_architecture.md §7`),
even though it has no OMG spec obligation — stability holds toward our
users, not toward the OMG community.

## 2 Non-OMG standards and external integration

| Standard | Purpose | Integration |
|---|---|---|
| **W3C Trace Context** | Distributed-tracing header propagation | As an RTPS ParameterList element in each sample (optional) |
| **OpenTelemetry** | Metrics, traces, logs emission | Native instrumentation in `zerodds-monitor`, OTLP export |
| **Prometheus text format** | Metrics scraping | Exporter in `zerodds-monitor` |
| **OpenSSL / BoringSSL / Rust Crypto** | Cryptographic primitives for DDS-Security | Plugin-based, swappable for EU crypto suites |
| **PKCS#11** | HSM integration for authentication | Optional feature in `zerodds-security` |
| **PlatformIO Library Registry** | Embedded distribution | Build pipeline in CI, `library.json` per release |
| **CycloneDX SBOM** | Software Bill of Materials | Per-release artifact, CI-generated |

## 3 Conformance profile goals

DDS DCPS 1.4 defines five conformance profiles. Our goals by phase:

| Profile | Phase 1 | Phase 2 | Phase 3 | Phase 4 |
|---|---|---|---|---|
| **Minimum Profile** | ✓ | ✓ | ✓ | ✓ |
| **Ownership Profile** | — | ✓ | ✓ | ✓ |
| **Content-Subscription Profile** | — | ✓ | ✓ | ✓ |
| **Persistence Profile** | — | — | ✓ | ✓ |
| **Object Model Profile** | — | — | — | (evaluate; low market relevance) |

XTypes conformance: we aim for **Complete** (Basic + Dynamic) by the end of Phase 2.

RTPS interoperability: **Basic Profile** from the end of Phase 1, **Minimal Profile** in the discovery area from the end of Phase 1.

## 4 Interoperability goals

Wire interop is validated continuously in CI and regularly at the OMG Plug-Fest against the following peers:

| Peer | Target version | Priority |
|---|---|---|
| Eclipse Cyclone DDS | current + LTS | High — default ROS 2 RMW |
| eProsima Fast DDS | current + LTS | High — default ROS 2 RMW from Humble |
| RTI Connext | 7.x LTS | High — market leader, defense relevance |
| OCI OpenDDS | current | Medium |
| TwinOaks CoreDX | current | Low — smaller user base |

Our OMG vendor ID is applied for in Phase 0. Before receiving it, we temporarily use the vendor-ID range for tests (two-digit developer IDs).

## 5 Explicitly out of scope

The following OMG specs and related standards are deliberately **not** implemented:

| Excluded | Rationale |
|---|---|
| DDS DLRL (Data Local Reconstruction Layer) | Largely orphaned spec part, no relevant deployments |
| CORBA / GIOP interop | Historical legacy, no customer demand |
| RTI FlatData, OpenSplice DDSI2E, vendor-specific extensions | Sovereignty principle: we stay with standard wire |
| JMS bridge | Not a strategic goal |
| Web integration (DDS-WebSocket, WebDDS prototypes) | Low priority, evaluate in a later phase |

## 6 Versioning policy

- Our product follows **SemVer** on all public API surfaces.
- OMG spec version pinning documented per release notes.
- Wire compatibility: **RTPS 2.5** is the minimum. Downgrade to 2.3/2.4 is supported via discovery negotiation where possible.
- **Safe-profile API stability:** From version 1.0, API changes in safe-subset crates are handled through a formal change-request procedure to minimize re-certification cost.

## 7 Spec-to-code traceability

Every OMG spec section is mapped to concrete code modules. The traceability matrix is maintained continuously (Claude-Teams-supported from git history and code annotations).

Format of the annotation in code:

```rust
/// Implements DDS-RTPS 2.5 §8.3.7.3 Heartbeat Submessage
#[spec(rtps = "2.5", section = "8.3.7.3")]
pub struct Heartbeat { ... }
```

These annotations are aggregated by the `zerodds-traceability` tool into a coverage matrix that serves as a basis both for internal reviews and for safety audits.
