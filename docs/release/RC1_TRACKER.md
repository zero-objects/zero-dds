# RC1 Tracker — Crate-by-Crate Walking-the-DAG

> **Live-Dokument.** Track-Materialisierung via git-commits.
> **Referenz:** `docs/release/RC1_GUARDRAILS.md` (DoD + Forbidden-Tokens + Public-Strategy).
> **Tag-Target:** `1.0.0-rc.1` pro Crate; Workspace-Final: `r1.0.0`.
> **Public-Mirror-Artifacts:** Public-Crates (🌐) materialisieren sich parallel als `github/crates/<crate>/` (→ `github.com/zero-objects/zero-dds`) und `website/docs/<crate>.md` (→ `zerodds.org`). Siehe Guardrails §1.12.

## Status-Symbole

- 📋 **todo** — noch nicht angefasst
- 🔄 **in-review** — Review läuft, Cleanup noch nicht abgeschlossen
- ✅ **rc1-ready** — DoD vollständig, Tag-fähig
- 🚫 **embargo** — bewusst zurückgehalten
- 🗑 **drop** — wird vor Release entfernt
- ⏸ **blocked** — wartet auf Lower-Layer-Crate

## Public-Strategy-Symbole

- 🌐 **public** — wandert ins externe Repo + crates.io
- 🔒 **public-feature-gated** — wandert raus, default-disabled
- 🚫 **embargo** — bleibt intern bis Trigger
- 🏠 **internal-only** — bleibt im Dev-Repo, kein Public-Mirror
- 🗑 **drop** — wird gelöscht

---

## Layer 0 — Foundation

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 0.1 | foundation | 🌐 | ✅ rc1-ready | Claude | [foundation.md](rc1-reviews/foundation.md) | Pilot done. F-001/F-002/F-003 alle als ✅ resolved in `RC1_FINDINGS.md`. |

## Layer 1 — Primitives

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 1.1 | cdr | 🌐 | ✅ rc1-ready | Claude | [cdr.md](rc1-reviews/cdr.md) | Spec: XTypes 1.3 §7.4 + §7.6.8 + IDL 4.2 §7.4.13. F-CDR-1 + F-CDR-2 dokumentiert (SPEC-MANDATED-OPEN / OPTIONAL-HOOK). |
| 1.2 | lint | 🏠 | ✅ rc1-ready | Claude | [lint.md](rc1-reviews/lint.md) | Internal Tooling. 7 Lints + 67 Tests + GitLab-CI + pre-commit hook. Keine Loose-Ends. |
| 1.3 | qos | 🌐 | ✅ rc1-ready | Claude | [qos.md](rc1-reviews/qos.md) | Spec: DDS 1.4 §2.2.3 + DDSI-RTPS §9.6.3.2. F-QOS-1/2/3 alle ✅ resolved (no_std + Exclusive-Ownership Cross-Layer-Wire-up + Pid-Konsolidierung mit rtps). |
| 1.4 | time-service | 🌐 | ✅ rc1-ready | Claude | [time-service.md](rc1-reviews/time-service.md) | Spec: OMG Time-1.1. F-TIMESVC-1 (no_std Warnings) ✅; F-TIMESVC-2 (Standalone, kein Wire-up-Gap, spec-distinkt zu DDS Time_t) ✅. |
| 1.5 | types | 🌐 | ✅ rc1-ready | Claude | [types.md](rc1-reviews/types.md) | Spec: DDS-XTypes 1.3. F-TYPES-1 (DynamicType-Bridge Union/Enum/Bitmask/Alias) ✅; F-TYPES-2 (no_std) ✅; F-TYPES-3 (Cross-Layer Wire-up: DdsType::TYPE_IDENTIFIER + PID_ZERODDS_TYPE_ID + TypeMatcher in wire_reader_to_remote_writer) ✅. |
| 1.6 | cdr-derive | 🌐 | ✅ rc1-ready | claude | [cdr-derive.md](rc1-reviews/cdr-derive.md) | Spec: zerodds-xcdr2-rust-1.0 §11.1. Proc-macro `#[derive(DdsType)]` + `#[dds(key)]`. 6 Tests gruen (byte-genau zu V-2). |

## Layer 2 — Wire

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 2.1 | discovery | 🌐 | ✅ rc1-ready | claude | [discovery.md](rc1-reviews/discovery.md) | SPDP+SEDP+TypeLookup+Security; F-DISC-1 (TypeLookup-Wiring in DCPS) als Cross-Layer-Finding für Layer-3-Review getrackt; 144+ tests grün |
| 2.2 | rtps | 🌐 | ✅ rc1-ready | claude | [rtps.md](rc1-reviews/rtps.md) | DDSI-RTPS 2.5 vollständig (K3b-Audit 121 done); 31 src-Files, 20 KLOC, 647 tests grün; 54 Phase-X-Marker bereinigt |
| 2.3 | transport | 🌐 | ✅ rc1-ready | claude | [transport.md](rc1-reviews/transport.md) | Trait-Crate; Locator-Re-Export aus rtps dokumentiert |
| 2.4 | transport-shm | 🌐 | ✅ rc1-ready | claude | [transport-shm.md](rc1-reviews/transport-shm.md) | POSIX-shm/mmap; Stub entfernt; ZeroDDS-SHM-Transport-1.0-Spec materialisiert; 18 tests grün |
| 2.5 | transport-tcp | 🌐 | ✅ rc1-ready | claude | [transport-tcp.md](rc1-reviews/transport-tcp.md) | RTPS-over-TCP per §9.4+§9.5; ZeroDDS-TCP-Transport-1.0-Spec materialisiert; Phase-2b-TODO-Marker entfernt; 55 tests grün |
| 2.6 | transport-tsn | 🌐 | ✅ rc1-ready | claude | [transport-tsn.md](rc1-reviews/transport-tsn.md) | OMG DDS-TSN 1.0 PIM+PSM+Config; 69 tests grün; cleanest crate |
| 2.7 | transport-udp | 🌐 | ✅ rc1-ready | claude | [transport-udp.md](rc1-reviews/transport-udp.md) | UDPv4 Unicast+Multicast, Deferral-Marker im Header gefixt; 11 tests grün |
| 2.8 | transport-uds | 🌐 | ✅ rc1-ready | claude | [transport-uds.md](rc1-reviews/transport-uds.md) | UDS Container-IPC; ZeroDDS-UDS-Transport-1.0-Spec materialisiert; 17 tests grün |

## Layer 3 — Schema

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 3.1 | idl | 🌐 | ✅ rc1-ready | claude | [idl.md](rc1-reviews/idl.md) | OMG IDL 4.2 / ISO 19516; Earley-Engine; 1047 tests grün; 604 Public-Items klassifiziert |
| 3.2 | idl-cpp | 🌐 | ✅ rc1-ready | claude | [idl-cpp.md](rc1-reviews/idl-cpp.md) | OMG IDL4-CPP + DDS-PSM-Cxx + DDS-RPC C++; 283 tests grün |
| 3.3 | idl-csharp | 🌐 | ✅ rc1-ready | claude | [idl-csharp.md](rc1-reviews/idl-csharp.md) | OMG IDL4-CSharp; 193 tests grün |
| 3.4 | idl-java | 🌐 | ✅ rc1-ready | claude | [idl-java.md](rc1-reviews/idl-java.md) | OMG IDL4-Java + DDS-Java-PSM; 260 tests grün |
| 3.5 | idl-rust | 🌐 | ✅ rc1-ready | claude | [idl-rust.md](rc1-reviews/idl-rust.md) | IDL4 → Rust DataTypes mit DdsType-Trait; 23 tests grün |
| 3.6 | idl-ts | 🌐 | ✅ rc1-ready | claude | [idl-ts.md](rc1-reviews/idl-ts.md) | DDS-TS 1.0 TypeScript-Codegen; 149 tests grün |
| 3.7 | xml | 🌐 | ✅ rc1-ready | claude | [xml.md](rc1-reviews/xml.md) | OMG DDS-XML 1.0 Parser + QoS-Profile-Loader; 302 tests grün |
| 3.8 | zerodds-xml-wire | 🌐 | ✅ rc1-ready | claude | [zerodds-xml-wire.md](rc1-reviews/zerodds-xml-wire.md) | DDS-XML Wire-PSM XML↔CDR-Codec; 40 tests grün |

## Layer 4 — Core Services

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 4.1 | dcps | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-01-dcps.md](rc1-reviews/04-01-dcps.md) | Spec: DDS-DCPS 1.4 |
| 4.2 | dcps-async | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-02-dcps-async.md](rc1-reviews/04-02-dcps-async.md) | Spec: zerodds-async-1.0 |
| 4.3 | flatdata | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-03-flatdata.md](rc1-reviews/04-03-flatdata.md) | Spec: zerodds-flatdata-1.0 |
| 4.4 | flatdata-derive | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-04-flatdata-derive.md](rc1-reviews/04-04-flatdata-derive.md) | Proc-Macro |
| 4.5 | monitor | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-05-monitor.md](rc1-reviews/04-05-monitor.md) | Built-in Monitor-Topics |
| 4.6 | observability-otlp | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-06-observability-otlp.md](rc1-reviews/04-06-observability-otlp.md) | OpenTelemetry-Sink |
| 4.7 | recorder | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-07-recorder.md](rc1-reviews/04-07-recorder.md) | .zddsrec Format |
| 4.8 | rpc | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-08-rpc.md](rc1-reviews/04-08-rpc.md) | Spec: DDS-RPC 1.0 |
| 4.9 | rt-linux | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-09-rt-linux.md](rc1-reviews/04-09-rt-linux.md) | Linux-RT-Scheduling |
| 4.10 | security | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-10-security.md](rc1-reviews/04-10-security.md) | Spec: DDS-Security 1.2 |
| 4.11 | security-crypto | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-11-security-crypto.md](rc1-reviews/04-11-security-crypto.md) | AES-GCM + HW-Detect |
| 4.12 | security-keyexchange | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-12-security-keyexchange.md](rc1-reviews/04-12-security-keyexchange.md) | DH-Keyexchange |
| 4.13 | security-logging | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-13-security-logging.md](rc1-reviews/04-13-security-logging.md) | Audit-Log-Sink |
| 4.14 | security-permissions | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-14-security-permissions.md](rc1-reviews/04-14-security-permissions.md) | Permissions-XML |
| 4.15 | security-pki | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-15-security-pki.md](rc1-reviews/04-15-security-pki.md) | X.509 + RSA-PSS |
| 4.16 | security-rtps | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-16-security-rtps.md](rc1-reviews/04-16-security-rtps.md) | RTPS-Header-AAD |
| 4.17 | security-runtime | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-17-security-runtime.md](rc1-reviews/04-17-security-runtime.md) | Plugin-Runtime |
| 4.18 | sql-filter | 🌐 | ✅ rc1-ready | claude | [rc1-reviews/04-18-sql-filter.md](rc1-reviews/04-18-sql-filter.md) | SQL92-Subset für ContentFilter |

## Layer 5 — Bridges

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 5.1 | amqp-bridge | 🌐 | ✅ rc1-ready | claude | [amqp-bridge.md](rc1-reviews/amqp-bridge.md) | OASIS AMQP 1.0 + DDS-AMQP-1.0 Wire-Codec, no_std + alloc; 188 Tests + 1 Doc-Test grün (82 unit + 90 boundary + 8 proptest + 8 fuzz-smoke); Type-System + Frame + 9 Performatives + 9 Message-Sections + Codec-Lite-Marker; 6 Sprint-Marker bereinigt, lib.rs-Header korrigiert (Performatives + Sections SIND abgedeckt); 0 Findings. |
| 5.2 | amqp-endpoint | 🌐 | ✅ rc1-ready | claude | [amqp-endpoint.md](rc1-reviews/amqp-endpoint.md) | OMG DDS-AMQP-1.0 Endpoint-Stack, no_std + alloc + Feature `std` für XML-Loader; 237 Tests grün (205 unit + 17 annex_a + 6 e2e + 4 fuzz + 6 proptest + 1 doc; +2 neu via Wire-up); SASL + Session/Link-Lifecycle + Routing + Mapping + Properties + DDS-Bridge + Annex-A; **F-AMQP-EP-DISPOSITION-MAPPER-WIRED** ✅ resolved (Trait war TEST-ONLY, jetzt produktiv via `LinkSession::settle_with_mapper`); 1 Sprint-Marker bereinigt. |
| 5.3 | coap-bridge | 🌐 | ✅ rc1-ready | claude | [coap-bridge.md](rc1-reviews/coap-bridge.md) | RFC 7252 + 7641 + 7959 + 6690 voll, no_std + alloc; 145 Tests grün (141 unit + 3 fuzz-smoke + 1 doc); Wire-Codec + Reliability + Block-Wise + CoRE-Link + Observe + Multicast + Caching + DTLS-Mode + DDS-Bridge; 1 Sprint-Marker bereinigt; 0 Findings. |
| 5.4 | grpc-bridge | 🌐 | ✅ rc1-ready | claude | [grpc-bridge.md](rc1-reviews/grpc-bridge.md) | gRPC HTTP/2 + gRPC-Web Wire-Codec, no_std + alloc; 60 Tests grün (54 unit + 5 fuzz-smoke + 1 doc); LPM + Path + Timeout + 17 Status-Codes + Custom-Metadata mit -bin-Suffix + Server-Skeleton; Tier-B (deps zerodds-http2 + zerodds-hpack); 0 Findings. |
| 5.5 | hpack | 🌐 | ✅ rc1-ready | claude | [hpack.md](rc1-reviews/hpack.md) | RFC 7541 HPACK no_std + alloc; 49 Tests + 1 Doc-Test grün; Static-Table (Appendix A) + Dynamic-Table (§4) + alle vier §6-Field-Repraesentationen + Huffman (Appendix B); 0 Findings. |
| 5.6 | http2 | 🌐 | ✅ rc1-ready | claude | [http2.md](rc1-reviews/http2.md) | RFC 9113 (HTTP/2) no_std + alloc; 45 Tests + 1 Doc-Test grün; Frame-Layer (§4) + alle 10 Frame-Types + Stream-State-Machine (§5.1) + Flow-Control (§5.2 + §6.9) + Connection-Preface (§3.4) + SETTINGS (§6.5) + Error-Codes (§7); RFC 7540 → 9113 Spec-Refs aktualisiert; 0 Findings. |
| 5.7 | mqtt-bridge | 🌐 | ✅ rc1-ready | claude | [mqtt-bridge.md](rc1-reviews/mqtt-bridge.md) | OASIS MQTT 5.0 no_std + alloc; 115 Tests grün (107 unit + 7 fuzz-smoke + 1 doc); Wire-Codec für alle 14 Control-Packets + 27 Properties + In-Memory-Broker + Topic-Filter + Keep-Alive + DDS-Bridge; 2 Sprint-Marker bereinigt; 0 Findings. |
| 5.8 | websocket-bridge | 🌐 | ✅ rc1-ready | claude | [websocket-bridge.md](rc1-reviews/websocket-bridge.md) | RFC 6455 + RFC 7692 voll, no_std + alloc; 155 Tests grün (150 unit + 4 fuzz-smoke + 1 doc); Wire-Codec + Handshake + Negotiation + Close + permessage-deflate + URI + UTF-8-Validator + DDS-Bridge; 0 Sprint-Marker pre-Review; 0 Findings. |
| 5.9 | zenoh-bridge | 🔒 | ✅ rc1-ready | claude | [zenoh-bridge.md](rc1-reviews/zenoh-bridge.md) | DDS↔Eclipse-Zenoh-Bridge, no_std + alloc default + Feature `zenoh-runtime` für Live-Pfad; 6 Tests grün (5 unit + 1 doc); Topic-Mapping + QoS-Translation pure-Rust + optionaler async Live-Runtime mit zenoh=1 + tokio; 0 Findings. |
| 5.10 | bridge-security | 🌐 | ✅ rc1-ready | claude | [bridge-security.md](rc1-reviews/bridge-security.md) | Bridge-Spec §7.1 TLS (rustls 0.23) + §7.2 Auth-Modes (none/bearer/jwt/mtls/sasl) + §7.3 Topic-ACL — gemeinsamer Substrat-Layer fuer alle 6 Bridge-Daemons; 43 Tests gruen (42 unit + 1 e2e TLS-Handshake); CONNECTED in ws + mqtt + coap + amqp + grpc + corba; 0 Findings; **Layer 5 = 10/10 ✅**. |

## Layer 6 — PSMs / Bindings

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 6.1 | cpp | 🌐 | ✅ rc1-ready | claude | [cpp.md](rc1-reviews/cpp.md) | C++17 RAII + DDS-PSM-Cxx 1.0 §7.5; 13 .hpp + smoke 10 sub-asserts grün |
| 6.2 | cs | 🌐 | ✅ rc1-ready | claude | [cs.md](rc1-reviews/cs.md) | C# P/Invoke + 22 QoS-Klassen + Listener-Bridge; ZeroDDS.Tests 8 grün |
| 6.3 | zerodds-c-api | 🌐 | ✅ rc1-ready | claude | [zerodds-c-api.md](rc1-reviews/zerodds-c-api.md) | extern "C"; 130+ FFI-fns; 63 cargo-tests grün; Coverage 23/23 done |
| 6.4 | java-omgdds | 🌐 | ✅ rc1-ready | claude | [java-omgdds.md](rc1-reviews/java-omgdds.md) | Pure-Java-Implementation; 37 cargo-tests grün |
| 6.5 | java | 🌐 | ✅ rc1-ready | claude | [java.md](rc1-reviews/java.md) | Container-Crate für Workspace-Build |
| 6.6 | java-omgdds | 🌐 | ✅ rc1-ready | claude | [java-omgdds.md](rc1-reviews/java-omgdds.md) | Pure-Java org.omg.dds.* + InProcessBus + Xcdr2Codec; 18 mvn + 1 cargo grün |
| 6.7 | py | 🌐 | ✅ rc1-ready | claude | [py.md](rc1-reviews/py.md) | PyO3 + Conditions/WaitSet + Status-Getter; 1 cargo-test grün |
| 6.8 | rs | 🌐 | ✅ rc1-ready | claude | [rs.md](rc1-reviews/rs.md) | Facade-Crate; 7 Re-Exports + 16 Top-Level; 3 cargo-tests grün |
| 6.9 | sys | 🌐 | ✅ rc1-ready | claude | [sys.md](rc1-reviews/sys.md) | Marker-Crate, verweist auf zerodds-c-api als C-FFI-Foundation |
| 6.10 | ts-node | 🌐 | ✅ rc1-ready | claude | [ts-node.md](rc1-reviews/ts-node.md) | DDS-PSM-Cxx-style API über koffi; 4 ts-tests grün |
| 6.11 | ts-wasm | 🌐 | ✅ rc1-ready | claude | [ts-wasm.md](rc1-reviews/ts-wasm.md) | wasm-bindgen XCDR1/XCDR2 codec |

## Layer 7 — Profiles

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 7.1 | conformance | 🌐 | ✅ rc1-ready | claude | [conformance.md](rc1-reviews/conformance.md) | Conformance-Test-Vector-Runner; 7 tests grün |
| 7.2 | zerodds-soap | 🌐 | ✅ rc1-ready | claude | [zerodds-soap.md](rc1-reviews/zerodds-soap.md) | DDS-Web SOAP-PSM; 37 tests grün |
| 7.3 | dlrl | 🌐 | ✅ rc1-ready | claude | [dlrl.md](rc1-reviews/dlrl.md) | DLRL 1.2; coverage 20 done + 4 n/a; 63 tests grün |
| 7.4 | dlrl-codegen | 🌐 | ✅ rc1-ready | claude | [dlrl-codegen.md](rc1-reviews/dlrl-codegen.md) | DLRL-Codegen; 15 tests grün |
| 7.5 | opcua-gateway | 🌐 | ✅ rc1-ready | claude | [opcua-gateway.md](rc1-reviews/opcua-gateway.md) | DDS-OPCUA 1.0; coverage 14 done + 8 n/a; 119 tests grün |
| 7.6 | rmw-zerodds-shim | 🌐 | ✅ rc1-ready | claude | [rmw-zerodds-shim.md](rc1-reviews/rmw-zerodds-shim.md) | ROS2-RMW Shim; 14 tests grün |
| 7.7 | ros2-rmw | 🌐 | ✅ rc1-ready | claude | [ros2-rmw.md](rc1-reviews/ros2-rmw.md) | REP-2007/2008/2009; coverage 13 done + 4 n/a; 56 tests grün |
| 7.8 | web | 🌐 | ✅ rc1-ready | claude | [web.md](rc1-reviews/web.md) | DDS-Web 1.0; coverage 16 done + 2 n/a; 70 tests grün |
| 7.9 | xrce | 🌐 | ✅ rc1-ready | claude | [xrce.md](rc1-reviews/xrce.md) | DDS-XRCE 1.0; coverage 82 done + 13 n/a; 329 tests grün |
| 7.10 | xrce-agent | 🌐 | ✅ rc1-ready | claude | [xrce-agent.md](rc1-reviews/xrce-agent.md) | XRCE-Agent; 13 tests grün |
| 7.11 | xrce-client | 🌐 | ✅ rc1-ready | claude | [xrce-client.md](rc1-reviews/xrce-client.md) | XRCE-Client; 9 tests grün |

## Layer 8 — CORBA-Stack

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| 8.1 | ami4ccm | 🌐 | ✅ rc1-ready | claude | [ami4ccm.md](rc1-reviews/ami4ccm.md) | OMG AMI4CCM 1.1 (formal/2015-08-03), no_std + alloc; 51 Tests grün (50 unit + 1 doc); Implied-IDL-Transformation (§7.3 + §7.5) + ExceptionHolder (§7.4) + Pragma-Parsing (§7.7) + Connector/Deployment/Multiplex (§7.6 + §7.8); 1 Sprint-Marker im Header bereinigt; OPTIONAL-HOOK-Klassifikation fuer CCM-Container-Konsumenten (Conformance-Punkt 1 voll, Connector-Hosting Caller-Layer). |
| 8.2 | ccm | 🌐 | ✅ rc1-ready | claude | [ccm.md](rc1-reviews/ccm.md) | OMG CCM 4.0 (formal/06-04-01) §6 + §13 + DDS4CCM 1.1 §6, no_std + alloc, dep zerodds-idl; 54 Tests grün (53 unit + 1 doc); Equivalent-IDL fuer Component/Home/EventType + Components::*-Valuetypes + LwCCM-Filter + PrimaryKey-Validate + DDS4CCM-Connector; OPTIONAL-HOOK fuer CCM-Container/Codegen-Konsumenten; §7-§16 als `n/a` begruendet (Container/ORB-Hosting). |
| 8.3 | corba-ccm | 🌐 | ✅ rc1-ready | claude | [corba-ccm.md](rc1-reviews/corba-ccm.md) | OMG CCM 4.0 (formal/2006-04-01) §6 + §7 + §13 voller Component-Container, no_std + alloc, Feature `cos-event` ist gewired zu corba-cos-event; 139 Tests grün (138 unit + 1 doc); CIDL + CIF + Component/Home + Container-Lifecycle + ORB-Extensions + PSS-Stub + Time-PSM + TimerEventService + Conformance-Markers; CONNECTED zu corba-ccm-lib + corba-ccm-ejb + corba-dnc + rtc. |
| 8.4 | corba-ccm-ejb | 🌐 | ✅ rc1-ready | claude | [corba-ccm-ejb.md](rc1-reviews/corba-ccm-ejb.md) | CCM↔EJB-Bridge (CCM 4.0 §16 + OMG TS 1.4 §10 + JEE JTA 1.3 §3.2 + JNDI 1.2), no_std + alloc; 25 Tests grün (24 unit + 1 doc); bijektives TxStatus↔JtaStatus-Mapping (10:10) + ConnectorBean-Lifecycle + JNDI↔CosNaming-Glue + Java-Bean-Stub-Codegen; 1 Sprint-Marker im Header bereinigt; OPTIONAL-HOOK fuer JEE-Container-Vendoren in Java-Schicht. |
| 8.5 | corba-ccm-lib | 🌐 | ✅ rc1-ready | claude | [corba-ccm-lib.md](rc1-reviews/corba-ccm-lib.md) | Production-ready CCM-Components-Library (CCM 4.0 §6 + §10), no_std + alloc; 24 Tests grün (23 unit + 1 doc); DdsBridgeComponent + PersistenceStorageComponent + TelemetryComponent; alle drei implementieren `corba-ccm::ComponentExecutor`. OPTIONAL-HOOK extern (Plan-referenced) + CONNECTED intern. |
| 8.6 | corba-codegen | 🌐 | ✅ rc1-ready | claude | [corba-codegen.md](rc1-reviews/corba-codegen.md) | OMG CORBA 3.3 Annex-A.1 IDL-Mapping-Codegen-Helpers, no_std + alloc; 17 Tests grün; F-CORBA-CODEGEN-NOT-WIRED ✅ resolved (4 Production-Refs in `corba-rust::{interface,valuetype,component}_emit` via `build_repository_id`). |
| 8.7 | corba-cos-event | 🌐 | ✅ rc1-ready | claude | [corba-cos-event.md](rc1-reviews/corba-cos-event.md) | OMG CosEventService v1.2 voller Stack, no_std + alloc; 24 Tests grün (23 unit + 1 doc); Push/Pull + EventChannelAdmin + TypedEvent; F-CORBA-COS-EVENT-NOT-WIRED ✅ resolved (Spec Time-Service §2.2.4 Wire-up via `corba-ccm::cos_event_bridge::EventChannelTimerCallback` Feature `cos-event` — TimerEventHandler ist `CosEventComm::PushConsumer`; +1 Cross-Crate-Test). |
| 8.8 | corba-cosnaming | 🌐 | ✅ rc1-ready | claude | [corba-cosnaming.md](rc1-reviews/corba-cosnaming.md) | OMG CosNaming 1.3 (formal/2004-10-03) Naming-Service, no_std + alloc; 26 Tests grün (25 unit + 1 doc); NamingContext + NamingContextExt In-Memory-Impl + alle 5 Exception-Klassen + Stringified-Name (§2.4) + corbaname-URL-Scheme (§2.5); ObjectRef-IOR-Inhalt CONNECTED zu corba-ior. |
| 8.9 | corba-csiv2 | 🌐 | ✅ rc1-ready | claude | [corba-csiv2.md](rc1-reviews/corba-csiv2.md) | OMG CORBA 3.3 Part 2 §10 CSIv2, no_std + alloc; 18 Tests grün (17 unit + 1 doc; +2 CDR-Roundtrip); AssociationOptions + CompoundSecMechList + GSSUP + SAS-Protocol + TLS-Mechanism-OID; F-CORBA-CSIV2-NOT-WIRED ✅ resolved intern (CDR-Encode/Decode → 4 Production-Refs auf zerodds-cdr) + extern (`corba-ior::StructuredComponent::CsiSecMechList` + Cross-Crate-Roundtrip-Test). |
| 8.10 | corba-dds-bridge | 🌐 | ✅ rc1-ready | claude | [corba-dds-bridge.md](rc1-reviews/corba-dds-bridge.md) | Bidirektionale CORBA↔DDS-Bridge (CORBA P1 §11 + P2 §15 + §13.6 + DDS 1.4 §2.2), no_std + alloc; 18 Tests grün (17 unit + 1 doc; +2 wire-Helpers neu); BridgeMapping + BridgeServant + LifecycleSync + wire::{decode_giop_request_bytes, object_key_from_ior}; **F-WORKSPACE-DEAD-DEPS-AUDIT Items 1+2 ✅ resolved** (corba-giop + corba-ior produktiv via wire-Modul gewired). |
| 8.11 | corba-dnc | 🌐 | ✅ rc1-ready | claude | [corba-dnc.md](rc1-reviews/corba-dnc.md) | OMG D&C 4.0 (formal/2006-04-02) voller Stack, no_std + alloc; 31 Tests grün (30 unit + 1 doc); Plan-Datenmodell DPD/CPD/IDD/PSD (§6 + §7) + XML-Plan-Loader (§10) + RepositoryManager (§8) + ExecutionManager + NodeManager (§9) + ContainerHost-Bridge zu corba-ccm. CONNECTED via ContainerHost; OPTIONAL-HOOK extern. |
| 8.12 | corba-giop | 🌐 | ✅ rc1-ready | claude | [corba-giop.md](rc1-reviews/corba-giop.md) | OMG CORBA 3.3 Part 2 §15 GIOP Wire-Codec, no_std + alloc, dep zerodds-cdr; 70 Tests grün (69 unit + 1 doc); alle 8 Message-Types fuer GIOP 1.0/1.1/1.2 inkl. Bidirectional-GIOP; OPTIONAL-HOOK fuer corba-iiop-Acceptor (Tier-B-Konsument). |
| 8.13 | corba-iiop | 🌐 | ✅ rc1-ready | claude | [corba-iiop.md](rc1-reviews/corba-iiop.md) | OMG CORBA 3.3 Part 2 §14 + §15.7 + §15.9 IIOP-TCP-Transport, no_std + alloc; 25 Tests grün (24 unit + 1 doc); ProfileBody alle 4 Versionen (1.0-1.3) + Connection mit Frame-Reader + Connector mit thread-safer Connection-Reuse-Pool + Acceptor + Bidirectional-GIOP. ProfileBody CONNECTED zu corba-ior. |
| 8.14 | corba-ior | 🌐 | ✅ rc1-ready | claude | [corba-ior.md](rc1-reviews/corba-ior.md) | OMG CORBA 3.3 Part 2 §13.6 voller IOR-Stack, no_std + alloc; 45 Tests grün (44 unit + 1 doc); IOR-Struct + alle Standard-Profile-Tags (inkl. IIOP-ProfileBody) + alle 32 Standard-TaggedComponents inkl. CSIv2-CompoundSecMechList-Wire-up + stringified-IOR (`IOR:hex`) + corbaloc/corbaname-URL-Parser. CONNECTED via corba-cosnaming + corba-iiop + corba-csiv2 + corba-dds-bridge. |
| 8.15 | corba-ir | 🌐 | ✅ rc1-ready | claude | [corba-ir.md](rc1-reviews/corba-ir.md) | OMG CORBA 3.3 Part 1 §14 Interface Repository, no_std + alloc; 20 Tests grün (19 unit + 1 doc); TypeCode (32 TCKinds) + Repository-Hierarchie + DefinitionKind + RepositoryId-Format. RepositoryId CONNECTED via corba-poa Servant-Wire-up; TypeCode/Repository als OPTIONAL-HOOK fuer IIOP-IR-Service-Konsumenten. |
| 8.16 | corba-poa | 🌐 | ✅ rc1-ready | claude | [corba-poa.md](rc1-reviews/corba-poa.md) | OMG CORBA 3.3 Part 1 §11 POA, no_std + alloc; 39 Tests grün (38 unit + 1 doc; +2 typisierte RepositoryId-Wire-up); alle 7 Policies + POAManager-State-Machine + Active-Object-Map + ServantManager. Servant::primary_repository_id + is_a_typed neu via corba-ir. |
| 8.17 | rtc | 🌐 | ✅ rc1-ready | claude | [rtc.md](rc1-reviews/rtc.md) | OMG RTC 1.0 (formal/2008-04-04), no_std + alloc; 48 Tests grün (47 unit + 1 doc); ReturnCode_t (§5.2.1) + LifeCycle-State-Machine (§5.2.2.3-§5.2.2.4) + ExecutionContext (§5.2.2.5-§5.2.2.6) + Periodic/Stimulus/Mode-Profile (§5.3) + Resource-Introspection (§5.4 Datenmodell) + Local PSM (§6.3); 4 Test-Mock-Stubs OK (cfg(test)); OPTIONAL-HOOK fuer RTC-Container-Konsumenten (z.B. OpenRTM-aist); §6.4/§6.5 als `n/a` (Container/ORB-Hosting), §5.4-Wire partial. |

## Embargo

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| E.1 | inspect-endpoint | 🚫 | 📋 todo | | | bis PDE-Release |

## Test/Chaos-Crates

| # | Crate | Public | Status | Reviewer | Review-Doc | Notes |
|---|---|---|---|---|---|---|
| T.1 | chaos-clock-skew | 🌐 | 📋 todo | | | Chaos-Engineering |

---

## Tools (kommen nach den Crates)

| # | Tool | Public | Status | Notes |
|---|---|---|---|---|
| TL.1 | admin | 🌐 | ✅ rc1-ready | claude | [admin.md](rc1-reviews/admin.md) | zerodds-admin CLI: domain inspect / qos validate / discovery snapshot |
| TL.2 | amqp-dds-endpoint | 🌐 | ✅ rc1-ready | claude | [amqp-dds-endpoint.md](rc1-reviews/amqp-dds-endpoint.md) | AMQP-Endpoint Server bridging OASIS AMQP 1.0 ↔ DDS |
| TL.3 | bench-suite | 🌐 | ✅ rc1-ready | claude | [bench-suite.md](rc1-reviews/bench-suite.md) | Benchmark suite: roundtrip-1us, transport throughput, RTPS-fragmentation perf |
| TL.4 | cargo-dag | 🏠 | ✅ rc1-ready | claude | [cargo-dag.md](rc1-reviews/cargo-dag.md) | Internal cargo-workspace DAG analyzer for publish-order resolution |
| TL.5 | chaos | 🌐 | ✅ rc1-ready | claude | [chaos.md](rc1-reviews/chaos.md) | Chaos-engineering CLI: tc proxy, partition simulator, endpoint-flap injector |
| TL.6 | dashboard | 🌐 | ✅ rc1-ready | claude | [dashboard.md](rc1-reviews/dashboard.md) | Live monitoring dashboard for a running DDS domain |
| TL.7 | dashboard-tauri | 🌐 | ✅ rc1-ready | claude | [dashboard-tauri.md](rc1-reviews/dashboard-tauri.md) | Tauri shell wrapping the dashboard for desktop deployment |
| TL.8 | idlc | 🌐 | ✅ rc1-ready | claude | [idlc.md](rc1-reviews/idlc.md) | zerodds-idlc IDL4 compiler with backends for C/C++/C#/Java/Python/Rust/TS |
| TL.9 | interop-matrix | 🏠 | ✅ rc1-ready | claude | [interop-matrix.md](rc1-reviews/interop-matrix.md) | Internal cross-vendor interop matrix renderer for the CI dashboard |
| TL.10 | isolation-smoke | 🏠 | ✅ rc1-ready | claude | [isolation-smoke.md](rc1-reviews/isolation-smoke.md) | Internal isolation-matrix smoke-test runner for CI |
| TL.11 | perf | 🏠 | ✅ rc1-ready | claude | [perf.md](rc1-reviews/perf.md) | Internal load generator + latency profiler + benchmark suite |
| TL.12 | pve | 🏠 | ✅ rc1-ready | claude | n/a (script-only) | Internal PVE/GLR lab-admin Bash script `glr-snapshot.sh`; not a Rust crate |
| TL.13 | qos-matrix | 🏠 | ✅ rc1-ready | claude | [qos-matrix.md](rc1-reviews/qos-matrix.md) | Internal QoS-policy compatibility matrix generator for docs |
| TL.14 | recorder-bridge | 🌐 | ✅ rc1-ready | claude | [recorder-bridge.md](rc1-reviews/recorder-bridge.md) | Recording bridge that captures DDS samples to disk for later replay |
| TL.15 | replay | 🌐 | ✅ rc1-ready | claude | [replay.md](rc1-reviews/replay.md) | Replay tool that reads recorded sessions and republishes samples |
| TL.16 | traceability | 🏠 | ✅ rc1-ready | claude | [traceability.md](rc1-reviews/traceability.md) | Internal requirements-to-code traceability matrix generator |
| TL.17 | xmlc | 🌐 | ✅ rc1-ready | claude | [xmlc.md](rc1-reviews/xmlc.md) | zerodds-xmlc DDS-XML 1.0 validator + schema checker + deployment renderer |

## Tutorials (kommen nach Tools)

Step-by-step learning curricula — strukturierte Einführung in
DDS-Konzepte über mehrere Kapitel/Lektionen.

| # | Tutorial | Public | Status | Notes |
|---|---|---|---|---|
| TU.1 | tutorials/dds-chat | 🌐 | ✅ rc1-ready | 15-Chapter-Curriculum + 10 Sprach-Ports + 7 Bridges + 3 Apps + Embedded-MCU. **Vollverifikation 2026-05-07**: 16 Rust-Crates ✅ (33 Tests in 7 integrations + 76 chapter+code-Tests + 03-rust-tui+embedded-mcu+code = 109+); 10 Sprach-Ports (cpp-qt6, cpp-tui, csharp-cli, csharp-wpf, java-backend, java-cli, python-cli, python-gui, ts-browser, ts-node) ✅ 73 Tests; 3 Non-Rust-Apps (flutter-mobile, qt-desktop, web-spa) ✅ 17 Tests. **Total ~199 Tests gruen**, alle Bridges on-wire-fähig gegen echte `crates/`-Bridge-Crates. |

## Demos (kommen nach Tutorials)

Full scenarios — End-to-End-Anwendungs-Showcases ohne Lehrbuch-
Struktur. „So sieht es aus, wenn alles zusammen läuft."

| # | Demo | Public | Status | Notes |
|---|---|---|---|---|
| DM.1 | demos/dds-warehouse | 🌐 | ✅ rc1-ready | 10-Stations Industrial-IoT-Hochregallager. **Vollverifikation 2026-05-07**: alle 10 Stations bauen + testen gegen ihre echten Service-Crates (dlrl, time-service, rtc, transport-tsn, xrce, opcua-gateway, ami4ccm, ccm, full corba-stack, corba-cos-event+web). **70 Stations-Tests + 7 Orchestrator-Tests = 77 gruen**. |
| DM.2 | demos/perf-camera-dds | 🌐 | 📋 todo | Flutter→WebSocket→DDS→Qt6 Performance-Demo. **Skeleton** — Architektur dokumentiert (`ARCHITECTURE.md`, ASCII-Diagramm), `idl/camera.idl` als Wire-Spec, aber `flutter-publisher/` und `qt-tileview/` enthalten nur READMEs ohne Code. Implementation als Folge-WP. |
| DM.3 | demos/otel | 🌐 | ✅ rc1-ready | OpenTelemetry Jaeger-Compose-Sample. **Vollverifikation 2026-05-07**: `docker compose -f jaeger-compose.yml config` validiert (ports 4317/4318/16686, jaegertracing/all-in-one:1.62). Konfig-Sample referenziert `OtlpExporter`-Default-Port 4318 aus `crates/observability-otlp/`. |

---

## Mainline-Doku (am Ende vor r1.0.0)

| Doc | Public | Status | Notes |
|---|---|---|---|
| README.md (Repo-Root) | 🌐 | 📋 todo | aktuell „Phase 0 Skelett" |
| CHANGELOG.md | 🌐 | 📋 todo | RC1-Entry + 1.0.0-Entry |
| CONTRIBUTING.md | 🌐 | 📋 todo | „internes Projekt" → public-OSS |
| SECURITY.md | 🌐 | 📋 todo | Public-Vulnerability-Disclosure |
| CODE_OF_CONDUCT.md | 🌐 | 📋 todo | Contributor Covenant 2.1 (existiert noch nicht) |
| LICENSE | 🌐 | ✅ done | Apache-2.0 Volltext eingesetzt |
| docs/architecture/00-09 | 🌐 | 📋 todo | Stand-Polish |
| docs/architecture/10_zero_principle_mapping.md | 🚫 | ✅ done | Embargo bis Zero-Principle public |

---

## Statistik (Live)

```
Layer 0:    1/1 ready  ✅ (foundation)
Layer 1:    6/6 ready  ✅ (cdr, lint, qos, time-service, types, cdr-derive)
Layer 2:    8/8 ready  ✅ (discovery, rtps, transport, transport-{shm,tcp,tsn,udp,uds})
Layer 3:    8/8 ready  ✅ (idl, idl-{cpp,csharp,java,rust,ts}, xml, zerodds-xml-wire)
Layer 4:   18/18 ready ✅ (dcps, dcps-async, flatdata, flatdata-derive, monitor, observability-otlp, recorder, rpc, rt-linux, security, security-{crypto,keyexchange,logging,permissions,pki,rtps,runtime}, sql-filter)
Layer 5:   10/10 ready ✅ (amqp-bridge, amqp-endpoint, bridge-security, coap-bridge, grpc-bridge, hpack, http2, mqtt-bridge, websocket-bridge, zenoh-bridge)
Layer 6:   11/11 ready ✅ (cpp, cs, zerodds-c-api, java-omgdds, java, java-omgdds, py, rs, sys, ts-node, ts-wasm)
Layer 7:   11/11 ready ✅ (conformance, zerodds-soap, dlrl, dlrl-codegen, opcua-gateway, rmw-zerodds-shim, ros2-rmw, web, xrce, xrce-agent, xrce-client)
Layer 8:   17/17 ready  ✅ (alle: ami4ccm, ccm, corba-ccm, corba-ccm-ejb, corba-ccm-lib, corba-codegen, corba-cos-event, corba-cosnaming, corba-csiv2, corba-dds-bridge, corba-dnc, corba-giop, corba-iiop, corba-ior, corba-ir, corba-poa, rtc)
Embargo:    0/1 ready
Test:       0/1 ready
─────────────────────
Crates:     90/91 ready

Tools:     17/17 ready ✅ (10 public: admin, amqp-dds-endpoint, bench-suite, chaos, dashboard, dashboard-tauri, idlc, recorder-bridge, replay, xmlc; 7 internal: cargo-dag, interop-matrix, isolation-smoke, perf, pve, qos-matrix, traceability)
Tutorials:  1/1 ready  ✅ (dds-chat — 199 Tests gruen, 10 Sprach-Ports + 3 Apps + 7 Bridges + Embedded-MCU)
Demos:      2/3 ready  🔄 (dds-warehouse, otel ready; perf-camera-dds Skeleton)
─────────────────────
Total Code: 89/112 ready

Mainline-Doku: 1/8 done
```

Update bei jedem `✅ rc1-ready`-Übergang.