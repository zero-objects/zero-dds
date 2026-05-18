# ZeroDDS v1.0 — Spec-Compliance-Fortschritt

**Stand:** 2026-04-26
**Quelle der Wahrheit:** `docs/plans/wp-spec-compliance-roadmap.md` (Master-Roadmap)
**Workspace:** 177+ Test-Suites grün, 0 failed, clippy + zerodds-lint clean.
**Status:** ✅ alle 7 Phasen + alle 8 Phase-6-Sub-Cluster done.
**Phase 2:** ✅ alle 10 Cluster voll done inkl. C2.2-b/-c.
**Phase 3:** ✅ alle 9 Cluster voll done inkl. C3.4-c (DCPS-Spawn
Stateless+VolatileSecure-Endpoints, 41 Tests).
**Phase 4:** ✅ alle 7 Cluster voll done inkl. C4.2-b (TypeLookup-
Auto-Trigger), C4.4-b (Built-in Types Auto-Register), C4.5-b (XML→
TypeObject-Bridge).
**Phase 5:** ✅ alle 5 Cluster voll done (C5.1-C5.4 + C5.5
Cross-Vendor 2026-04-26).
**Phase 7:** ✅ 4/4 done (C7.A Foundation, C7.B QoS-Profile-Library,
C7.C Domain/Participant/Application Library, C7.D Types-XML +
Sample-Codec).

## Phasen-Übersicht

| Phase | Titel | Cluster | Done | Offen | Status |
|---|---|---:|---:|---:|---|
| **1** | Wire-Grundlagen | 10 | 10 | 0 | ✅ **abgeschlossen** |
| **2** | DCPS-API-Vollständigkeit | 10 | 10 | 0 | ✅ **abgeschlossen** |
| **3** | Security-Hardening | 9 | 9 | 0 | ✅ **abgeschlossen** |
| **4** | Type-System-Vervollständigung | 7 | 7 | 0 | ✅ **abgeschlossen** |
| **5** | Sprach-Bindings (parallel zu 4) | 5 | 5 | 0 | ✅ **abgeschlossen** |
| **6** | Erweiterungen (RPC + XRCE) | 8 | 8 | 0 | ✅ **abgeschlossen** (C6.1.D JNI-Wiring stretch) |
| **7** | DDS-XML-Konfiguration | 4 | 4 | 0 | ✅ **abgeschlossen** |

\* C2.2 Listener-Slots + Bubble-Up als Folge-Stufe in Phase 3.

---

## Phase 1 — Wire-Grundlagen ✅

Alle Cluster auf main, gepusht, 0 failed.

| Cluster | Inhalt | Tests |
|---|---|---|
| **C1.1** | RTPS ProtocolVersion 2.5 Default | (existing) |
| **C1.2** | XCDR2-Encoding-Stack vollständig (LC0..7, EncapsulationKind, PID_IGNORE) | 13 LC + 11 encap |
| **C1.3** | HeaderExtension + CRC-32C/64/MD5 + Must-Understand-Reject | 21 foundation/CRC + 50 RTPS + 2 Cyclone |
| **C1.4** | Must-Understand-Bit + PID-Reject-Logik | (in C1.3 enthalten) |
| **C1.5** | KeyHash XCDR2 + PID_KEY_HASH + Inline-QoS-Slot | 13 KeyHash + 5 keyed-DdsType |
| **C1.6** | GAP/HEARTBEAT GroupInfo + filteredCount + GSN | (in C1.E) |
| **C1.7** | InfoSource/InfoReply Decoder + ReceiverState | (in C1.E) |
| **C1.8** | Reliable HEARTBEAT FinalFlag-Default + Pre-Emptive ACKNACK | (in C1.E) |
| **C1.9** | Builtin-Endpoint-Set Bits 10/11/16-27/28/29 | 44 unit + 3 integration |
| **C1.10** | ParticipantMessageData / WLP / MANUAL-LIVELINESS Wire | (in C1.9 enthalten) |

**Acceptance:** Cyclone-Live-Discovery + XCDR2-Pub/Sub byte-identisch (bestehender WP 0.6 + WP 1.4 SEDP).

---

## Phase 2 — DCPS-API-Vollständigkeit ✅

Alle 10 Cluster auf main.

| Cluster | Inhalt | Tests |
|---|---|---|
| **C2.1** | Entity-Lifecycle (Trait + Impls für alle 6 Entity-Typen + Mutex<QoS> + StatusCondition) | 8 unit + 13 integration |
| **C2.2** | 13 Status-Structs + 6 Listener-Traits + Listener-Slots auf allen 6 Entitaeten + Bubble-Up-Dispatcher (C2.2-b) | 14 status + 11 listener + 27 dispatch + 21 integration + 8 entity-slots |
| **C2.3** | WaitSet + GuardCondition + Condition-Trait + StatusCondition | 10 |
| **C2.4** | SampleInfo (alle 12 Felder) + Instance-Lifecycle (register/unregister/dispose/get_key_value/lookup) + State-Machine | 47 |
| **C2.5** | TopicDescription-Trait + find_topic + ContentFilteredTopic mit SQL-Filter | 22 |
| **C2.6** | Builtin-Topic-Reader (DCPSParticipant/Topic/Publication/Subscription) + Discovery-Hook | 37 |
| **C2.7** | ignore_participant/topic/publication/subscription + delete_contained_entities + get_discovered_* | 21 |
| **C2.8** | compute_compatibility (9-Policy-Aggregat) + HISTORY/RES_LIMITS-Konsistenz + ExclusiveOwnership-Resolver | 14 |
| **C2.9** | Coherent-Sets (PID_COHERENT_SET/GROUP_COHERENT_SET/GROUP_SEQ_NUM) + GroupAccessScope | 8 |
| **C2.10** | InstanceHandle + HANDLE_NIL + Time/Duration + IDL-PSM-Konstanten + ReturnCode-Erweiterungen | 20 |

**Folge-Stufe C2.2-b** ✅ done (Listener-Slot-Integration + Bubble-Up,
Sub-Agent 2026-04-26, 56 Tests neu).
**Folge-Stufe C2.2-c** ✅ done (alle 8 Status-Kind-Trigger verdrahtet:
inconsistent_topic, offered/requested_deadline_missed, offered/requested_
incompatible_qos, liveliness_lost/changed, sample_lost/rejected; neuer
Writer-Liveliness-Watchdog. 25 Tests neu.).

---

## Phase 7 — DDS-XML 🟡

| Cluster | Inhalt | Status |
|---|---|---|
| **A** Foundation | XSD-Loader + Datentypen + Inheritance + DTD-Verbot | ✅ done (48 Tests, 98% Coverage) |
| **B** QoS-Profile-Library | `<qos_library>` + `<qos_profile>` + 22 Policies + Inheritance + Topic-Filter + Single-QoS-Shortcut | ✅ done (40+ Tests) |
| **C** Domains/Participants/Applications | `<domain_library>` + `<domain_participant_library>` + `<application_library>` + Top-Level `DdsXml` + Cross-Library-Resolve + Participant-Inheritance + DCPS-Adapter-Trait-Skeleton | ✅ done (23 Integration + Inline-Tests) |
| **D** Types-XML + Sample-Codec | XTypes-XML mit Module/Struct/Enum/Union/Typedef/Bitmask/Bitset + Sample-Codec (Struct/Union/Sequence/Array/Primitives) | ✅ done (26+ Tests) |

---

## Phase 3 — Security-Hardening (Next)

Aus zerodds-security-1.2.open.md sind kritische Issues identifiziert:

| Cluster | Inhalt | Pflicht | Risiko | Aufwand | Status |
|---|---|---|---|---|---|
| **C3.1** | PKI-Handshake-Vollständigkeit (hash_c1/c2 + Cert-Bind + Signature) | mandatory | blocker (MitM) | L | ✅ done |
| **C3.2** | Permissions-CA-Sig-Verify + S/MIME-Envelope | mandatory | blocker (Forge) | L | ✅ done |
| **C3.3** | Plugin-Class-Id-Versionierung (`:1.2`-Suffix) | mandatory | blocker (Cross-Vendor-Match) | S | ✅ done |
| **C3.4** | DCPSParticipantStatelessMessage + VolatileMessageSecure Topics + DCPS-Spawn (C3.4-a/-b/-c) | mandatory | blocker | L | ✅ done (C3.4-c 2026-04-26: SecurityBuiltinStack mit BestEffort+Reliable Endpoint-Pair, Wire-Demux-Hook im Metatraffic-Pfad, 41 Tests) |
| **C3.5** | IdentityToken/PermissionsToken in SPDP-PIDs (0x1001/0x1002) | mandatory | blocker | M | ✅ done |
| **C3.6** | CryptoAlgorithmId Wire-Konflikt-Fix (Spec-Tab. 22) | mandatory | **breaking-Wire** | S | ✅ done |
| **C3.7** | session_key HMAC-Derivation + AAD-Format | mandatory | blocker | M | ✅ done |
| **C3.8** | Secure-Builtin-Endpoints + GUID-Anti-Squatter | mandatory | hoch (DoS) | M | ✅ done |
| **C3.9** | PSK-Builtins (DDS-Security 1.2 §10.7-9) | mandatory | none | M | ✅ done |

**Acceptance:** Cyclone-Live-Interop für Security-Topics.

---

## Phase 4 — Type-System-Vervollständigung

| Cluster | Inhalt | Aufwand |
|---|---|---|
| **C4.1** | DynamicType-API (Builder, DynamicData, MemberDescriptor, TypeDescriptor) | XL | ✅ done (Foundation: 18 TypeKinds + Builder + 12 typed Accessoren + Loans + Bridge zu TypeObject; 40 Integration + 33 Inline-Tests) |
| **C4.2** | TypeLookup-Service (Builtin-Endpoints, getTypes/Dependencies) + Auto-Trigger (C4.2-b) | L | ✅ done (TypeRegistry + Server + Client + Pagination + Service-Instance-Name; C4.2-b 2026-04-26: Auto-Trigger via on_remote_publication/subscription_discovered + Backoff 5s/3 Versuche + Reply-Ingest + Re-Match-Hook; 13 Tests neu) |
| **C4.3** | Built-in Annotations Apply-Logik (@ignore_literal_names, @verbatim, @data_representation, @topic) | M | ✅ done (`apply_to_member` + `apply_to_type` Bridge IDL→DynamicType-Descriptor; alle 13 Builtin-Annotations gemappt; Passthrough-Report fuer @verbatim/@unit/@hashid/@bit_bound/@autoid; 16 Tests) |
| **C4.4** | Built-in Types Set + Auto-Register (C4.4-b) | M | ✅ done (DDS::String/KeyedString/Bytes/KeyedBytes als DynamicType-Singletons; C4.4-b 2026-04-26: Auto-Register im Participant.new() + idempotenter unregister_builtin_types-Disable; 12 Tests neu) |
| **C4.5** | XML-Schema-Loader (XTypes Annex A) + XML→TypeObject-Bridge (C4.5-b) | M | ✅ done (URI-Loader + Strict/Lax-Validation; C4.5-b 2026-04-26: XmlType→MinimalTypeObject mit struct/enum/union/typedef/sequence/array/bitmask/bitset, EquivalenceHash byte-identisch zu IDL-Lowering, 33 Tests) |
| **C4.6** | IDL-4.2-Spec-Treue (Konstanten-Eval, Resolver, Anon-Types-AST) | L | ✅ done (Phase-1: §1.1 Const-Eval, §1.4 Name-Resolver, §1.5 Forward-Decl-Completion, §1.6 Anon-Types, §1.7 Builtin-Annotation-Lowering, §1.8 Union-Valid, §1.9 Bitfield-Valid, §1.13 Preprocessor-Hardening) |
| **C4.7** | TryConstruct-Semantik | S | ✅ done (Discard/UseDefault/Trim mit Bound-Violation-Detection für String/Sequence/Array; Apply im DynamicData::set-Pfad; 13 Tests) |

---

## Phase 5 — Sprach-Bindings (parallel zu Phase 4)

| Cluster | Inhalt | Aufwand |
|---|---|---|
| **C5.1** | IDL4-CPP Codegen + omg::types Header | XL — ✅ C5.1-a/b done (Blocks A-H, 135+ Tests, `crates/idl-cpp/`) |
| **C5.2** | DDS-PSM-CXX Binding | L — ✅ done (Header-Skeleton-Layer + 5 Templates, 11 Integration-Tests) |
| **C5.3** | IDL4-CSharp Codegen + ISequence-Runtime | L — ✅ done (C5.3-a Foundation 91 Tests + C5.3-b 53 Tests: ISequence/IBoundedSequence-Runtime, 7 Annotation-Bridges, ITopicType-Marker; `crates/idl-csharp/`) |
| **C5.4** | IDL4-Java Codegen + DDS-Java-PSM | L — ✅ done (C5.4-a Cluster A-D 95 Tests + C5.4-b 59 Tests: Bitset/Bitmask via EnumSet, Multi-Inh via Companion-Interface, @value(N)-Enum, 7 Annotation-Bridges, TopicType-Marker; `crates/idl-java/`) |
| **C5.5** | Cross-Vendor-Validierung (FastDDS + Cyclone) | M — ✅ done (12 FastDDS-Live-Tests + 2 Cyclone-Lueckenfueller + 2 deterministische Gap-Tests, `live-interop`-Feature in zerodds-discovery + zerodds-dcps, `crates/discovery/tests/common/cross_vendor.rs` + `crates/dcps/tests/common/cross_vendor.rs` Helper, Doku in `docs/spec-coverage/cross-vendor-validation.md`. RTI bleibt out-of-scope mangels Lizenz.) |

### C5.5 — Run-Anleitung

Live-Tests laufen nur mit `LLVM_HOST_AVAILABLE=1` + `--features
live-interop` + `-- --ignored`:

```bash
# alle Live-Tests gegen llvm@llvm
LLVM_HOST_AVAILABLE=1 cargo test -p zerodds-dcps -p zerodds-discovery \
    --features live-interop -- --ignored --nocapture

# pro Test-File einzeln
cargo test -p zerodds-dcps --features live-interop \
    --test fastdds_live_pub -- --ignored --nocapture
cargo test -p zerodds-dcps --features live-interop \
    --test fastdds_live_sub -- --ignored --nocapture
cargo test -p zerodds-dcps --features live-interop \
    --test fastdds_qos_matrix -- --ignored --nocapture
cargo test -p zerodds-discovery --features live-interop \
    --test fastdds_live_spdp -- --ignored --nocapture

# deterministische Gap-Tests (laufen ohne Lab)
cargo test -p zerodds-dcps --test cyclone_live_wlp_manual -- --ignored
cargo test -p zerodds-discovery --test cyclone_typelookup_responder
```

Voraussetzungen auf llvm: FastDDS 2.9 + Cyclone 0.10.2, `sshpass`
lokal, `ip link set enp6s18 allmulticast on` auf der VM.

Details: `docs/spec-coverage/cross-vendor-validation.md`.

---

## Phase 6 — Erweiterungen 🟡

| Cluster | Inhalt | Aufwand |
|---|---|---|
| **C6.1.A** | RPC Common-Types + IDL-Annotations + Topic-Naming + Service-Mapping | M — ✅ done 2026-04-26 (`crates/rpc/`, 56 Tests) |
| **C6.1.B** | RPC Codegen Request/Reply-Pairs + PIDs 0x0080-0x0083 + PID_RELATED_SAMPLE_IDENTITY | L — ✅ done 2026-04-26 (Codegen-Daten Basic+Enhanced, 4 PIDs Pub/Sub-Roundtrip, Inline-QoS RELATED_SAMPLE_IDENTITY E2E, RpcEndpointBuilder; 46 Tests) |
| **C6.1.C** | RPC Requester/Replier-Runtime + QoS-Profile-Resolution | XL — ✅ done 2026-04-26 (Requester sync+blocking, Replier tick-driven, RpcQos Spec §7.11, XML-Profile-Resolver, SampleIdentity-Korrelation, 54 Tests) |
| **C6.1.D** | RPC PSM-Bindings (C++/Java) | XL — ✅ done 2026-04-26 (C++-PSM `crates/idl-cpp/src/rpc.rs` + 4 Templates dds::rpc::{Future,Promise,Requester,Replier,RemoteException,ServiceTraits}, 30 Tests; Java-PSM `crates/idl-java/src/rpc.rs` + 12 Runtime-Files org.zerodds.rpc.{RemoteException,UserException,SampleIdentity,Requester,Replier,Holder,Future,ServiceContext}, 35 Tests; Out/InOut via Holder<T>; JNI-Live-Wiring deferred bis Pilot-Kunde) |
| **C6.2.A** | XRCE Wire-Lite (16 Submessages, RFC-1982 SerialNumber, UDP-Mapping) | L — ✅ done 2026-04-26 (`crates/xrce/`, 94 Tests) |
| **C6.2.B** | XRCE Object-Model + Reliable-Stack + Continuous-Read | L — ✅ done 2026-04-26 (13 Object-Kinds + ObjectStore Reuse/Replace, Reliable-Sender/Receiver mit Window-Cap + Bitmap-ACKNACK, FRAGMENT-Reassembler mit DoS-Caps, Continuous-Read DeliveryControl, TransportLocator Small/Medium/Large, Multicast-Discovery; 79 Tests) |
| **C6.2.C** | XRCE XML/File-Configuration | M — ✅ done 2026-04-26 (XrceConfig-Loader fuer §9.3-Hierarchie, Bridge zu `crates/xml/` fuer QoS-Profiles + TypeObject, to_create_messages topologisch, 42 Tests) |
| **C6.2.D** | XRCE TCP/Serial Transports + TLS/DTLS | L — ✅ done 2026-04-26 (TCP §11.3 voll mit 2-Byte-Length-Prefix, Serial Annex C HDLC-Framer mit CRC-16-CCITT-FALSE + Byte-Stuffing, TLS Skeleton, DTLS Trait + DummyDtls, TransportLocatorTcp/Serial; 95 Tests) |

---

## Aufwands-Aggregat

- **Erledigt:** Phase 1 + Phase 2 + Phase 7 Cluster F = ~7-9 PM
- **Verbleibend:** Phase 3 (4-5 PM) + Phase 4 (3-4 PM) + Phase 5 parallel (9-12 PM) + Phase 6 (5-7 PM) + Phase 7 G-H-I (1-2 PM)
- **Gesamt v1.0:** ~30-40 PM. Mit 3-4 Devs und Parallelisierung **~9-12 verbleibende Kalendermonate**.

## Test-Counts (Workspace, Stand main)

128+ Test-Suites, ~2500+ Tests, 0 failed. Pro Crate (Auswahl):

| Crate | Lib-Tests |
|---|---:|
| zerodds-cdr | 102+ |
| zerodds-rtps | 380+ |
| zerodds-dcps | 200+ |
| zerodds-discovery | 50+ |
| zerodds-qos | 195+ |
| zerodds-types | 186+ |
| zerodds-xml | 48 |
| zerodds-security-runtime | 160+ |
| zerodds-security-permissions | 83+ |
| zerodds-security-pki | 36+ |
| zerodds-security-crypto | 16+ |
| zerodds-foundation | 21+ |

## Pipeline

GitLab gitlab.sandra-kessler.eu — Runner glr1 (network_mode=host).
Status: grün nach jedem Push der WP-X.Y-Stufen.
