# RC1 Review — `zerodds-transport-tsn`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 2.6 (Wire — TSN PIM + PSM)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

OMG DDS-TSN 1.0 (formal/2024-05-16) Configuration-Modell PIM (§7.2) +
DDSI-RTPS-Ethernet-PSM (Annex A) + XML/JSON-Configuration-PSM (§7.3).
Pure-Rust no_std + alloc.

## 2 Public-Strategy

🌐 public — DDS-TSN 1.0 ist OMG-Standard-Erweiterung.

## 3 Content-Inventur

### 3.1 Module

```
src/lib.rs              # Re-Exports
src/mac.rs              # MacAddress (Tab 7.20)
src/vlan_tag.rs         # Ieee802VlanTag (Tab 7.21)
src/dscp.rs             # Dscp (RFC 2474)
src/traffic.rs          # TrafficSpecification (Tab 7.16)
src/time_aware.rs       # TimeAware (Tab 7.17)
src/stream.rs           # TsnTalker, TsnListener, StreamIdentifier (Tab 7.15+7.24)
src/data_frame.rs       # DataFrameSpecification (Tab 7.19)
src/ethernet_psm.rs     # EthernetFrameHeader (Annex A)
src/config.rs           # XML/JSON-Configuration-PSM (§7.3)
src/pim/                # PIM Application/Deployment-Modell (§7.2.1)
```

60 Public-Items insgesamt.

### 3.2 Public-API-Surface

Aufgeschlüsselt nach Family in §3.4.

### 3.3 Tests

- `cargo test -p zerodds-transport-tsn`: ✅ 69 passed.
- `cargo build --no-default-features --features alloc`: ✅ baut.

### 3.4 Coherence-Audit (§1.5b) — gruppiert nach Public-API-Family

| Family | Items | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|---|
| **PIM Wire-Types** | `MacAddress`, `Ieee802VlanTag`, `TPID_8021Q`, `TPID_8021AD`, `Dscp`, `TimeAware`, `TrafficSpecification`, `TransmissionSelection`, `DataFrameSpecification`, `IPv4Tuple`, `IPv6Tuple`, `StreamIdentifier`, `TsnTalker`, `TsnListener` | DDS-TSN 1.0 §7.2 Tab 7.15-7.24 (PIM Configuration-Tables) | 0 ext (Library-Public-API), End-User der TSN-Configurator-Tooling baut | SPEC-MANDATED Public-API (XML/JSON-Config-Konsumenten + End-User) | doc-as-hook |
| **Ethernet-PSM** | `EthernetFrameHeader`, `ETHERTYPE_RTPS` | DDS-TSN 1.0 Annex A | 0 ext (Library-Public-API für Custom-PSM-Konsumenten) | SPEC-MANDATED Public-API | doc-as-hook |
| **Configuration-PSM** | `parse_xml_config`, `render_json_config`, `ConfigError`, `TsnConfiguration`, `DeploymentLibrary`, `DomainLibrary`, `DomainEntry`, `QosLibrary`, `QosProfile`, `QosProfileEntry`, `TsnQosLibrary`, `TalkerEntry`, `ListenerEntry`, `TopicLibraryEntry`, `RegisteredType`, `*QosRef`-Types (5×) | DDS-TSN 1.0 §7.3 (Configuration-Schema XML+JSON) | 0 ext (XML/JSON-Loader-Public-API für End-User), 11+ docs | SPEC-MANDATED Public-API | doc-as-hook |
| **PIM Application/Deployment** | `Application`, `ApplicationLibrary`, `Domain`, `DomainParticipant`, `DomainParticipantLibrary`, `Node`, `NodeLibrary`, `Deployment`, `DeploymentConfiguration`, `DdsTsnConfig`, `IpV4`, `IpV6`, `MacAddr` | DDS-TSN 1.0 §7.2.1 (PIM Application-Modell) | `Application` 39, `Domain` 58, `DomainParticipant` 39, `Node` 25, etc. | CONNECTED (39+ ext refs in tests/cross-vendor + tools) | — |
| **Configuration-PSM Renderers (json/xml-mod)** | `render_dds_tsn_json`, `parse_dds_tsn_xml`, `RenderJsonError`, `ParseXmlError`, `DeploymentValidationError` | DDS-TSN 1.0 §7.3 (PIM-Renderer-Helper) | 0 ext direkt; via `parse_xml_config`/`render_json_config` indirekt CONNECTED | VENDOR-EXTENSION (granulare Public-API-Helper) | doc-as-hook |
| **VLAN/Ethernet Errors** | `VlanError`, `EthError`, `DscpError` | Vendor-Error-Types | `VlanError` 0 ext; aber Return-Type pub-Konstruktor-Methoden | VENDOR-EXTENSION (Error-Contract) | — |

**Zusammenfassung:** 60/60 Public-Items klassifiziert in 6 Families.
0 ❌-Klassen.

**Spec-Conformance-Argument:** DDS-TSN 1.0 §7.2 spezifiziert die PIM-
Tables explizit als Wire-Format-Public-Vocabulary für Bridge-Vendoren
(Cisco IE / Hirschmann / etc.) und End-User-Configurator-Tooling. Die
"OVER-EXPOSED"-Klassifikation ist daher nicht zutreffend — das sind
SPEC-MANDATED Public-API-Items. Public-API-Reduktion auf `pub(crate)`
würde die Spec-Konformität brechen.

## 4 Wiring

### 4.1 Dependencies

```toml
roxmltree = "0.20"
```

(Kein zerodds-internal-Dep — TSN ist Pure-Library.)

### 4.2 Dependents

Keine Production-Konsumenten in `crates/`. TSN-Crate ist Standalone-
Library für End-User-TSN-Configurator-Tooling und Bridge-Vendor-Konsum.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std + roxmltree |
| `alloc` | ✅ (via std) | no_std-Build mit Heap |

## 5 Spec-Relevanz

- **OMG DDS-TSN 1.0** (formal/2024-05-16) — `docs/spec-coverage/dds-tsn-1.0.md`.
  - §7.2 Configuration-Modell PIM (Tab 7.15-7.24): ✅ done
  - §7.3 XML/JSON-Configuration-PSM: ✅ done
  - Annex A DDSI-RTPS-Ethernet-PSM: ✅ done
- **Caller-Layer** (außerhalb Crate-Scope): TSN-UNI-Wire-Protocol (vendor-spez.),
  YANG-PSM (separate Crate), Hardware-TX-Timestamping (OS-API), gPTP-Daemon
  (linuxptp/ptp4l).

## 6 Cleanup-Findings

Keine — cleanest crate aller Layer-2-Crates beim Layer-2-Pass-1-Sweep
(0 Phase-Marker, 0 Forbidden-Tokens).

## 7 Cleanup-Actions

1. SPDX-Header in 14 src-Files (10 root + 4 pim-Submodule).
2. Cargo.toml RC1-Metadata.
3. README + CHANGELOG.

## 8 Spec-Doc-Updates

`docs/spec-coverage/dds-tsn-1.0.md` aktualisiert (Layer-2-Pass-1).

## 9 Doc-Artefacts

- [x] Cargo.toml RC1
- [x] lib.rs-Header mit voll-spezifischer §-Mapping
- [x] README mit Plattform-/Spec-Tabelle

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-transport-tsn                                  # ✅ 69 passed
cargo clippy -p zerodds-transport-tsn --all-targets -- -D warnings   # ✅
cargo doc -p zerodds-transport-tsn --no-deps                         # ✅
cargo build -p zerodds-transport-tsn --no-default-features --features alloc  # ✅
```

## 11 RC1-DoD-Checkliste

- [x] §1.1-§1.13 alle ✅

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
