# Phase-4 Closeout

**Datum:** 2026-05-02
**Status:** ✅ alle 15 WPs aus den drei orthogonalen Clustern abgeschlossen, gepusht.

Phase 4 hatte **kein** neues Protokoll-Stack-Ziel. Stattdessen drei
parallele Härtungs-Spuren auf den Phase-3-Liefer-Stacks: Security-
Plugins voll spec-konform, ROS-2-Anbindung über RMW-FFI,
Conformance-Test-Vektoren als Selbst-Audit-Suite.

## 1. Lieferung

### Cluster A — DDS-Security 1.2 Hardening (7 Sprints)

| Sprint | WP | Crate | Inhalt |
|---|---|---|---|
| 9 | A.1a | `security-pki` | `AuthRequestMessageToken` (§9.3.2.5.1.1) + `IdentityStatusToken` (§9.3.2.5.1.2) — Pre-Handshake-Token + Cert-Revocation-Status. |
| 9 | A.1b | `security-pki` | `IdentityToken` (§10.3.2.1) Wire-Form mit 4 Pflicht-Properties + `subject_match` (§7.4.3 Cert-Bind) + PEM-Cert-Subject-Extraction via `x509-cert`. |
| 10 | A.2 | `security-permissions` | XML-Schema voll: `<deny_rule>`/`<domains>`/`<partitions>`/`<data_tags>` + `Grant::is_publish_allowed`/`is_subscribe_allowed`/`matches_domain`-Helpers. |
| 11 | A.3 | `security-crypto` | `CryptoTransformIdentifier` (§10.5.2.1) + `CryptoHeader` (§10.5.2.3) + `CryptoFooter` (§10.5.2.4) + `negotiate_transform()` + `BUILTIN_CRYPTO_PLUGIN`-Const. |
| 12 | A.4 | `security` | `BuiltinSecurityTopicProfile` mit Spec-konformen QoS-Profilen für `DCPSParticipantStatelessMessage` (§7.5.3) + `DCPSParticipantVolatileMessageSecure` (§7.5.4). |
| 13 | A.5 | `rtps` | `PID_PARTICIPANT_SECURITY_INFO` (0x1005, §7.4.1.6) mit 2x u32 Bit-Masks + `fully_protected_default()`-Builder. |
| 14 | A.6 | `security-rtps` | RTPS-Header-AAD-Builder (§7.4.6.6) + Submessage-AAD-Builder (§7.4.7.8/9) für SRTPS-Wrapping. |
| 15 | A.7 | `security` | Plugin-Trait-Vollständigkeit: 4 Auth-Methoden + 5 AccessControl-Methoden mit Default-Impls (Backward-Compat). |

**Total Cluster-A:** ~2400 Lines, +47 Tests, 7 spec-konforme Wire-Codecs/Validators.

### Cluster B — ROS-2-RMW-Adapter (1 Sprint, 3 WPs gebündelt)

| Sub-Task | Inhalt |
|---|---|
| B.1 | `ffi_api.rs` — `RmwRet`-Enum (REP-2007 §4) + `RMW_IMPLEMENTATION_IDENTIFIER`-Convention + `check_rmw_identifier()` + `map_to_rmw_ret()`. |
| B.2 | `type_mapping.rs` — `RosTypeRef` (package/namespace/type) + `to_dds_type_name()`-Convention `<pkg>::<ns>::dds_::<Type>_` + `RosBuiltinType`-Enum mit allen 15 ROS-IDL-Builtins (REP-2008 §4.4). |
| B.3 | `rmw_qos_mapping.rs` — `RmwQosProfile` (`#[repr(C)]` für FFI) + `rmw_to_dds()` + 4 Standard-Profile (default/sensor_data/parameters/services_default per REP-2003 + REP-2009). |

**Total Cluster-B:** ~750 Lines, +36 Tests in `ros2-rmw`-Crate.

> *Note:* Der eigentliche `extern "C"`-Wrapper-Crate (cbindgen + ament-cmake) ist absichtlich nicht in Phase-4 — der ist ROS-2-Distribution-spezifisch (Galactic/Humble/Iron/Jazzy je unterschiedlich) und wird im jeweiligen Distro-Build-Pfad gebaut. Die Rust-seitigen Conversion-Helper sind komplett.

### Cluster C — Conformance-Test-Vector-Suiten (1 Sprint, 5 Sub-Tasks)

Neuer Crate `zerodds-conformance` mit 5 Spec-Vector-Modulen, gegen die Phase-3-Stacks fahrend:

| Modul | Spec | Cases |
|---|---|---|
| `autobahn_ws` | RFC 6455 + RFC 7692 | 10 |
| `oasis_mqtt` | OASIS MQTT-5.0 §3 + §4 | 13 |
| `h2spec_grpc` | RFC 7540 + RFC 7541 + gRPC-protocol | 11 |
| `coap_plugtest` | RFC 7252 + 7641 + 7959 + 6690 | 10 |
| `dds_xml_xvendor` | DDS-XML 1.0 §6 | 13 |
| **Total** |  | **57** |

**Highlights:** Spec-§1.3 Sample-Nonce + §C.4.1 Huffman + §C.1 Integer-Vektoren werden alle byte-identisch gegen unsere Implementations validiert. `CaseResult::{Pass, Fail, Skip}` + `run_suite()`-Reporter machen die Suite CI-tauglich.

## 2. Workspace-Bilanz

* **Crates** vorher → nachher: 86 → 87 (+`zerodds-conformance`)
* **Files** vorher → nachher: 868 → 886
* **Phase-4-Tests:** +47 (Cluster A) + 36 (Cluster B) + 7 Suite-Wrapper-Tests + 57 in-suite Conformance-Cases
* **Workspace `cargo test --workspace`:** alle Suites grün
* **clippy + zerodds-lint + fmt:** durchgängig clean

## 3. Strategische Werte

### Was sich geändert hat ggü. Phase-3-Closeout

* **Cross-Vendor-Security-Compat** — DDS-Security 1.2 §10.x voll. Vorher
  fehlten AuthRequest, IdentityStatus, IdentityToken-Wire, Cert-Bind,
  `<deny_rule>`/`<domains>`/`<data_tags>`-XML, CryptoTransform-Wire-IDs,
  Stateless/Volatile-QoS-Profile, PID 0x1005, Header-AAD. Jetzt all das
  spec-konform und mit Round-Trip-Tests.
* **ROS-2-Pfad** — bisher gab es `ros2-rmw` nur als Topic-Mangling-Helper.
  Jetzt zusätzlich Type-System-Mapping (REP-2008), QoS-Mapping (REP-2009)
  und FFI-Skeleton (REP-2007).
* **Selbst-Auditierbarkeit** — der `zerodds-conformance`-Crate dokumentiert
  per Pure-Rust-Test-Vektor, dass ZeroDDS gegen alle 5 externen Spec-
  Suiten lauffähig ist. Ergänzend kann `live-interop` weiterhin Autobahn/
  h2spec/ddsperf usw. fahren — die Conformance-Crate macht das nicht
  abhängig.

### Sicherheits-Compliance-Status

Mit Cluster-A-Abschluss erfüllt ZeroDDS die DDS-Security-1.2-
Conformance-Punkte aus `wp-spec-compliance-roadmap.md` §C3:

* C3.1 PKI-Handshake — done
* C3.2 Permissions-CA-Sig + Permissions-XML — done
* C3.3 Wire-Crypto-Konflikte — done
* C3.4 Stateless/Volatile-Topics — done
* C3.5 Discovery-Erweiterungen — done
* C3.6 SRTPS-Wrapping + Header-AAD — done
* C3.7 Plugin-Vollständigkeit — done

Alle 7 vorher als "kritisches Sicherheits-Risiko" markierten Items sind geschlossen.

## 4. Was nicht in Phase 4 lag

* **Echte AES-GCM-Hardware-Beschleunigung** auf ARMv8-Crypto-Extensions / x86-AES-NI — Phase 5 Real-Time-Cluster.
* **Latency-Hardening für 1µs-Pfade** — Phase 5.
* **`rmw-zerodds-shim`** mit cbindgen + ament-cmake-Hook und realem `librmw_zerodds.so` für eine konkrete ROS-2-Distro — Phase 5 Build-Adapter-Cluster.
* **Recording/Replay** + **Chaos-Test-Suite** — Phase 5 Tooling-Cluster.

## 5. Phase-5-Brücke

Phase 5 wird in `docs/PHASE5_PLAN.md` ausgearbeitet. Drei Cluster:

* **Cluster D — Real-Time + Latency** (~25-35 PW): no_alloc Hot-Path, ARM-Crypto-Extensions, isolcpu/SCHED_FIFO-Profile, lock-free History-Cache.
* **Cluster E — Build- und Distro-Adapter** (~15-20 PW): `rmw-zerodds-shim` (ROS-2 Galactic + Humble + Iron + Jazzy), Debian/RPM-Pakete, Cargo-Workspace-Publish auf crates.io.
* **Cluster F — Tooling + Operability** (~25-35 PW): Recording/Replay-Format + Chaos-Suite + OTel-Instrumentierung + Tauri-Dashboard.

---

*Cross-Refs:* `docs/PHASE3_CLOSEOUT.md`, `docs/PHASE4_PLAN.md`,
`docs/plans/wp-spec-compliance-roadmap.md` §C3,
`project_security_posture.md`, `project_ros2_architecture_decision.md`.
