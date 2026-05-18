# Phase-4 Plan

**Stand:** 2026-05-01
**Vorgaenger:** `docs/PHASE3_CLOSEOUT.md` (Sprint 1-8 abgeschlossen).

Phase 4 fokussiert auf **Hardening, Adapter, Conformance** — nicht
mehr auf neue Protokoll-Stacks. Drei Cluster, parallelisierbar:

```
Cluster A — Security-Hardening
    ↓ blockt nichts
Cluster B — ROS-2-RMW-Adapter         ── parallel zu A
    ↓ blockt nichts
Cluster C — Conformance-Test-Suites   ── parallel zu A+B
```

**Total Phase-4:** ~75-95 PW menschen-äquivalent.

---

## 1. Cluster A — DDS-Security 1.2 Hardening

**Aufwand:** ~30-40 PW.
**Ziel:** alle offenen Items aus `docs/plans/wp-spec-compliance-roadmap.md`
§C3 schliessen — kritische Sicherheits-Risiken-Pflicht-Items.

### A.1 — PKI-Handshake-Vollständigkeit (WP 3.A)
* OMG DDS-Security 1.2 §10.3.4 (Authentication-Plugin: 3-Way-
  Handshake mit `auth_request` + `handshake_request` +
  `handshake_reply` + `handshake_final`).
* §7.4.3 Cert-Bind: X.509-Cert-Chain-Validation + Subject-Match
  gegen Permissions-Subject.
* §10.3.2 Token-Strukturen: vollständige `IdentityToken`,
  `IdentityStatusToken`, `HandshakeMessageToken`-Implementations.

### A.2 — Permissions-CA-Sig + Permissions-XML-Voll (WP 3.B)
* §10.4.1.1 Permissions-CA-Cert-Chain-Validation (gegen
  configured Trust-Store).
* §10.4.1.3 Permissions-Document-XML-Schema voll
  (Domain-Rule + Topic-Rule + Tag-Liste + Validity-Range).

### A.3 — Wire-Crypto-Konflikte (WP 3.C)
* §10.5.2-3 Crypto-Token-Exchange.
* §8.1 Tab.22 Crypto-Plugin-Pluggability.
* §7.3.20 Builtin-Crypto-Plugin (AES-GCM 128/256).
* §10.3.2.1 Cross-Vendor-Wire-Compat (Cyclone-Security-
  Test-Vektoren).

### A.4 — Stateless/Volatile-Topics (WP 3.D)
* §7.5.3 ParticipantStatelessMessage-Topic.
* §7.5.4 ParticipantVolatileMessageSecure-Topic mit
  Crypto-Receiver-Specific-Macros.

### A.5 — Discovery-Erweiterungen Security (WP 3.E)
* §7.5.1.4-8 Permissions-Token + IdentityToken-Distribution
  via Builtin-Topics.
* §7.4.7.1 Authentication-Builtin-Topic.

### A.6 — SRTPS-Wrapping + RTPS-Header-AAD (WP 3.F)
* §7.4.6.6 SRTPS-Submessage-Wrap.
* §7.4.7.8/9 SubmessageProtection + DataProtection.
* §8.1 RTPS-Header-AAD (Authenticated-Additional-Data).

### A.7 — Plugin-Vollständigkeit (WP 3.G)
* §9.3.2 Authentication-Plugin-Trait voll.
* §9.4.2 AccessControl-Plugin-Trait voll.
* §9.5.1 Crypto-Plugin-Trait voll.

**Cross-Vendor-Tests:** Cross-handshake gegen Cyclone DDS Security
+ FastDDS Security in `tests/interop/security_*.sh`.

---

## 2. Cluster B — ROS-2-RMW-Adapter

**Aufwand:** ~20-25 PW.
**Ziel:** ZeroDDS als RMW-Implementation für ROS 2 nutzbar machen,
**ohne** rclcpp/ROS-2-Stack zu reimplementieren — nur die FFI-
Adapter-Schicht `rmw_zerodds`.

### B.1 — `rmw_zerodds`-Crate (FFI)
* `crates/rmw-zerodds/`: extern-C-API kompatibel mit ROS 2
  REP-2007 (rmw API).
* Build-Output: `librmw_zerodds.so` für AMD64 + ARM64 + RISC-V.
* CMake/colcon-Integration via Crate-Build-Script.

### B.2 — REP-2008 (Type-System-Mapping)
* ROS-2-IDL → DDS-XTypes-Mapping (Spec §2.4 + §3 IDL-Annotation-
  Konventionen).
* `idl-ros2`-Crate: Codegen-Backend für ROS-2-IDL-Subset
  (rosidl_generator_cpp/_python).

### B.3 — REP-2009 (QoS-Mapping)
* ROS-2-QoS-Profile (`rmw_qos_profile_t`) → DDS-QoS-Tabelle.
* Reliability/Durability/History/Deadline/Liveliness/Lifespan-
  Mapping.

### B.4 — Discovery-Inter-Op
* `ros_discovery_info`-Topic + Sedp-Bindings.
* Cyclone-DDS-Discovery-Format ↔ ZeroDDS-SEDP byte-identisch.

### B.5 — Cross-Stack-Tests
* `tests/interop/ros2_smoke.sh`: ROS-2 demos (talker/listener)
  gegen ZeroDDS-RMW.
* `colcon test` mit `RMW_IMPLEMENTATION=rmw_zerodds`.

---

## 3. Cluster C — Conformance-Test-Suites

**Aufwand:** ~25-30 PW.
**Ziel:** externe Conformance-Suites integrieren — gibt uns
"third-party-validated"-Stempel auf jedem Stack aus Phase 3.

### C.1 — WebSocket Autobahn
* `crates/websocket-bridge`: Autobahn-Test-Suite-Runner-Integration
  (alle 517 Test-Cases inkl. permessage-deflate).
* CI-Job `ci/jobs/autobahn.yml`.

### C.2 — MQTT-5.0 OASIS-Test-Vectors
* OASIS MQTT-5.0 Conformance-Test-Suite (2019-Edition).
* Broker-Tests gegen `mqtt-bridge::Broker`.
* Client-Tests gegen `mqtt-bridge::Client` (existiert noch nicht —
  C.2.b liefert Client als Sub-WP).

### C.3 — gRPC-Test-Suite
* gRPC interop-Tests (Google-Reference-Suite + grpc-go-Tests
  als Cross-Vendor).
* HTTP/2 h2spec-Tool integriert.

### C.4 — CoAP-Plugtest-Vektoren
* IETF-CoAP-Plugtest-Vektoren als Test-Inputs.
* Block-Wise Test-Suite (RFC 7959).
* Observe-Test-Suite (RFC 7641).

### C.5 — DDS-XML-Cross-Vendor
* `zerodds-xml-wire`: Cross-Vendor-Tests gegen Cyclone-DDS-XML-Output
  und FastDDS-XML-Output.
* XSD-Schema-Validation gegen W3C-XSD-Test-Suite.

---

## 4. Aufwands-Bilanz

| Cluster | Aufwand (PW) | Sequentialitaet |
|---|---|---|
| A — Security | 30-40 | parallel |
| B — RMW | 20-25 | parallel |
| C — Conformance | 25-30 | parallel |
| **Total** | **75-95** | parallel |

Mit drei Spuren parallelisiert ist die End-to-End-Dauer ~ max der
einzelnen Cluster + 10% Coordination = ~5-6 PM bei 1.5 FTE.

---

## 5. Sprint-Phasing

Wie Phase-3: ein Sprint = ein Cluster-WP = ein produktreifer
Release. Keine MVPs.

```
Sprint 9:  A.1 PKI-Handshake voll                  ── ~5-7 PW
Sprint 10: A.2 + A.3 Permissions + Wire-Crypto     ── ~6-9 PW
Sprint 11: A.4 + A.5 Stateless/Volatile + Disco    ── ~5-7 PW
Sprint 12: A.6 + A.7 SRTPS + Plugin                ── ~6-9 PW
Sprint 13: B.1 + B.2 RMW-Skeleton + Type-System    ── ~7-10 PW
Sprint 14: B.3 + B.4 + B.5 RMW-QoS + Disco + Tests ── ~8-10 PW
Sprint 15: C.1 + C.4 Autobahn + CoAP-Plugtest      ── ~5-7 PW
Sprint 16: C.2 + C.3 + C.5 OASIS + gRPC + DDS-XML  ── ~10-12 PW
```

Cross-Sprint-Abhaengigkeiten: keine. Cluster sind orthogonal.

---

## 6. Phase-4-Acceptance

Phase 4 ist abgeschlossen, wenn:

* Alle 7 Security-WPs voll implementiert + Cross-Vendor-Wire-
  Compat-Tests gegen Cyclone-Security + FastDDS-Security gruen.
* `rmw_zerodds` lädt in einer ROS-2-Galactic/Humble/Iron-Distro
  und besteht talker/listener + lifecycle-Demo.
* Autobahn-Test-Suite alle 517 Tests grün.
* OASIS MQTT-5.0-Conformance grün.
* gRPC-interop-Tests gegen grpc-go grün.
* CoAP-Plugtest-Vektoren grün.

Cross-Refs: `docs/PHASE3_CLOSEOUT.md`,
`docs/plans/wp-spec-compliance-roadmap.md`,
`project_security_posture.md`,
`project_ros2_architecture_decision.md`.
