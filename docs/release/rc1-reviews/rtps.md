# RC1 Review — `zerodds-rtps`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 2.2 (Wire — DDSI-RTPS Stack)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

DDSI-RTPS 2.5 Wire-Stack — Submessages, Writer/Reader-State-Machines,
Reliable + Fragmentation, Inline-QoS, ParameterList, BuiltinTopicData,
HEADER_EXTENSION, Group-Digest. Pure-Rust no_std + alloc,
`forbid(unsafe_code)`.

## 2 Public-Strategy

🌐 public — Wire-Stack ist Public-Library für DDS-Anwendungen + End-User-
Custom-Wire-Builders.

## 3 Content-Inventur

### 3.1 Module

31 src-Files, ~20 KLOC, 166 Public-Items insgesamt:

```
src/
├── lib.rs
├── error.rs               # WireError
├── wire_types.rs          # Guid, EntityId, SequenceNumber, Locator, ...
├── header.rs              # RtpsHeader (20B Wire)
├── header_extension.rs    # HEADER_EXTENSION-Submessage (§9.4.2.15)
├── submessage_header.rs   # SubmessageHeader (4B)
├── submessages.rs         # DATA, DATA_FRAG, HEARTBEAT, ACKNACK, GAP, INFO_*
├── datagram.rs            # encode/decode RTPS-Messages
├── parameter_list.rs      # PL_CDR_LE ParameterList + 54 PIDs
├── inline_qos.rs          # Inline-QoS-Helpers (RelatedSampleIdentity, ...)
├── group_digest.rs        # GroupDigest_t (§8.3.5.10)
├── history_cache.rs       # HistoryCache + LockFreeReadHistoryCache
├── message_builder.rs     # OutboundDatagram-Aggregation
├── fragment_assembler.rs  # Reader-side Reassembly (DoS-Caps)
├── writer.rs              # BestEffortWriter
├── reader.rs              # BestEffortReader
├── reader_proxy.rs        # ReaderProxy
├── writer_proxy.rs        # WriterProxy
├── reliable_writer.rs     # ReliableWriter (tick-driven)
├── reliable_reader.rs     # ReliableReader
├── reliable_stateless_writer.rs  # SPDP-Writer-Variante
├── receiver_state.rs      # ReceiverState
├── qos_bridge.rs          # SEDP-PubData ↔ WriterQos/ReaderQos
├── participant_data.rs    # ParticipantBuiltinTopicData (SPDP)
├── publication_data.rs    # PublicationBuiltinTopicData (SEDP)
├── subscription_data.rs   # SubscriptionBuiltinTopicData (SEDP)
├── participant_message_data.rs  # WLP-Heartbeat (§9.6.3.1)
├── participant_security_info.rs # PID 0x1005 (DDS-Security 1.2 §7.4.1.6)
├── endpoint_security_info.rs    # Endpoint-Security-Attributes
├── security_algo_info.rs        # DDS-Security 1.2 §7.4.7.1.6
├── property_list.rs       # PID_PROPERTY_LIST (DDSI-RTPS §9.6.3.2)
```

### 3.2 Tests

- `cargo test -p zerodds-rtps`: ✅ 647 passed.

### 3.3 Coherence-Audit (§1.5b) — gruppiert nach Public-API-Family

Bei einer Wire-Spec-Library wie `zerodds-rtps` ist jedes Spec-mandated
Wire-Item per Definition Public-API. Die "OVER-EXPOSED"-Klassifikation
ist daher in den meisten Fällen unzutreffend — das sind SPEC-MANDATED
Public-API-Konstanten und -Typen für End-User-Custom-Wire-Builders.

| Family | Items | Spec-Anker | External Refs (Production) | Klassifikation | Decision |
|---|---|---|---|---|---|
| **Wire-Types Core** | `Guid`, `EntityId`, `SequenceNumber`, `Locator`, `ProtocolVersion`, `VendorId`, `LocatorKind`, `EntityKind`, `FragmentNumber`, `UExtension4`, `WExtension8`, `SPDP_*`-Constants | DDSI-RTPS 2.5 §8.3.2 (Wire-Types-Definition) | massiv (50+ Konsumenten in dcps, discovery, transport-*, security, etc.) | CONNECTED + SPEC-MANDATED | — |
| **RTPS-Header** | `RtpsHeader`, `RTPS_MAGIC` | DDSI-RTPS 2.5 §8.3.3 | dcps, discovery | CONNECTED + SPEC-MANDATED | — |
| **Submessage-Header** | `SubmessageHeader`, `SubmessageId`, `SubmessageId`-Discriminator-Vals | DDSI-RTPS 2.5 §8.3.4 | dcps, discovery, security-rtps | CONNECTED + SPEC-MANDATED | — |
| **Submessage-Bodies** | `DataSubmessage`, `DataFragSubmessage`, `HeartbeatSubmessage`, `HeartbeatFragSubmessage`, `AckNackSubmessage`, `NackFragSubmessage`, `GapSubmessage`, `InfoTimestampSubmessage`, `InfoSourceSubmessage`, `InfoReplySubmessage`, `GapGroupInfo`, `HeartbeatGroupInfo` | DDSI-RTPS 2.5 §8.3.7 (10 Submessage-Typen) | dcps, discovery, security-rtps | CONNECTED + SPEC-MANDATED | — |
| **Submessage-Flags** | `ACKNACK_FLAG_FINAL`, `DATA_FLAG_DATA/INLINE_QOS/KEY/NON_STANDARD`, `DATA_FRAG_FLAG_*`, `GAP_FLAG_*`, `HEARTBEAT_FLAG_*`, `INFO_REPLY_FLAG_MULTICAST`, `INFO_TIMESTAMP_FLAG_INVALIDATE`, `SUBMESSAGE_FLAG_MUST_UNDERSTAND` | DDSI-RTPS 2.5 §8.3.7 Tab 8.X (Flag-Bit-Masken) | 0 ext direkt; Wire-Format-Vocabulary für End-User | SPEC-MANDATED Public-API (Wire-Format-Konstanten) | doc-as-hook |
| **SequenceNumberSet / FragmentNumberSet** | `SequenceNumberSet`, `FragmentNumberSet`, `RTPS_BITMAP_MAX_BITS` | DDSI-RTPS 2.5 §8.3.5.5+§8.3.5.7 | dcps, discovery | CONNECTED | — |
| **Datagram-Encode/Decode** | `encode_data_datagram`, `decode_datagram`, `ParsedSubmessage`, `ParsedDatagram` | DDSI-RTPS 2.5 §8.3.6 | dcps, discovery, security-rtps | CONNECTED | — |
| **HEADER_EXTENSION (§9.4.2.15)** | `SUBMESSAGE_ID_HEADER_EXTENSION`, `HE_FLAG_E/C0/C1/C_MASK/L/P/U/V/W`, `HeTimestamp`, `ChecksumKind`, `ChecksumValue`, `MAX_HE_LENGTH`, `pid_must_understand`, `pid_strip`, `PID_MUST_UNDERSTAND`, `PID_VENDOR_SPECIFIC` | DDSI-RTPS 2.5 §9.4.2.15 (RTPS 2.5 Header-Extension-Container) | 0 ext direkt; Wire-Format-Public-API | SPEC-MANDATED Public-API (Wire-Format-Konstanten) | doc-as-hook |
| **ParameterList** | `ParameterList`, `Parameter`, `pid::*` (12 PIDs re-exportiert aus qos + 40 native PIDs), `is_standard_pid`, `MAX_PARAMETERS`, `MUST_UNDERSTAND_BIT`, `VENDOR_SPECIFIC_BIT` | DDSI-RTPS 2.5 §9.4.2.11 + §9.6.3.2 | dcps, discovery, security-rtps | CONNECTED | — |
| **Inline-QoS (Optional Submessage-Inline-Parameters)** | `directed_write_*`, `original_writer_info_*`, `related_sample_identity_param`, `find_directed_write`, `find_original_writer_info`, `find_related_sample_identity`, `find_status_info`, `lifecycle_inline_qos`, `reply_inline_qos`, `status_info_param`, `SAMPLE_IDENTITY_WIRE_SIZE`, `SampleIdentityBytes` | DDSI-RTPS 2.5 §9.6.3.7-§9.6.3.13 (Inline-QoS-PIDs) | 0 ext direkt; aufgerufen via DCPS-`subscriber.rs` (interne Routine, internal=many) | VENDOR-EXTENSION (Inline-QoS-Helpers für End-User-Read-Path-Custom-Filtering) | doc-as-hook |
| **GroupDigest** | `GroupDigest` | DDSI-RTPS 2.5 §8.3.5.10 | dcps (intern verwendet via Hash-Computation-Pfad) | VENDOR-EXTENSION (Public-Type für End-User-Hash-Inspect) | doc-as-hook |
| **HistoryCache** | `HistoryCache`, `LockFreeReadHistoryCache`, `CacheChange`, `CacheError`, `HistoryCacheSnapshot`, `HistoryCacheStats`, `LockFreeInner` | DDSI-RTPS 2.5 §8.2.10 | dcps, security-rtps | CONNECTED | — |
| **Message-Builder** | `MessageBuilder`, `OutboundDatagram`, `AddError` | Vendor-Aggregation für Send-Tick | dcps | CONNECTED | — |
| **Fragment-Assembler** | `FragmentAssembler`, `AssemblerCaps`, `CompletedSample`, `DropReason`, `DEFAULT_MAX_FRAGMENT_SIZE`, `DEFAULT_MAX_PENDING_SNS`, `DEFAULT_MAX_SAMPLE_BYTES` | DDSI-RTPS 2.5 §8.3.7.3 (DATA_FRAG-Reassembly + DoS-Caps) | dcps | CONNECTED + VENDOR-EXTENSION (Defaults als Public-Inspect-Konstanten) | — |
| **Writer/Reader (Best-Effort)** | `BestEffortWriter`, `BestEffortReader` | DDSI-RTPS 2.5 §8.4.x (Stateless-Best-Effort-Variante) | 0 ext direkt; via DCPS-Runtime indirekt | VENDOR-EXTENSION (Public-Library-API für End-User-Custom-RTPS-Stack-Builders) | doc-as-hook |
| **Reliable Writer/Reader** | `ReliableWriter`, `ReliableWriterConfig`, `ReliableReader`, `ReliableReaderConfig`, `ReliableStatelessWriter`, `ReliableStatelessStats`, `WriterProxyState`, `ReaderProxy`, `WriterProxy`, `ReceiverState` | DDSI-RTPS 2.5 §8.4.x (Reliable-State-Machines) | dcps, discovery (SEDP-Reliable + SPDP-Stateless) | CONNECTED | — |
| **QoS-Bridge** | `qos_bridge::*` | DDS 1.4 §2.2.3 + DDSI-RTPS §9.6.3.2 (SEDP→QoS-Wire-Mapping) | 0 ext direkt; intern via PublicationBuiltinTopicData::as_writer_qos | VENDOR-EXTENSION (Public-Helper für End-User-QoS-Inspect aus SEDP-Daten) | doc-as-hook |
| **BuiltinTopicData (SPDP/SEDP)** | `ParticipantBuiltinTopicData`, `PublicationBuiltinTopicData`, `SubscriptionBuiltinTopicData`, `endpoint_flag::*` (16 Bit-Konstanten), `Duration` (re-export aus qos), `Locator`-Helpers, `encode_octet_seq_le`, `ContentFilterProperty`, `decode_content_filter_property`, `encode_content_filter_property_le` | DDSI-RTPS 2.5 §8.5.3 + §8.5.4 + §9.3.2.12 + DDS 1.4 §2.2.5 | dcps, discovery | CONNECTED | — |
| **Participant-Message-Data (WLP)** | `ParticipantMessageData`, `MAX_DATA_LEN`, `PARTICIPANT_MESSAGE_DATA_KIND_*`, `ENCAPSULATION_CDR_BE/LE/CDR2_BE/CDR2_LE` | DDSI-RTPS 2.5 §8.4.13 + §9.6.3.1 + XTypes 1.3 §7.4.1.1 | dcps (WLP), `ENCAPSULATION_CDR2_*` jetzt CONNECTED in dcps::strip_user_encap | CONNECTED + SPEC-MANDATED | — (DEAD-Fix Layer-2-Pass-2: CDR2-Konstanten in dcps konsumiert) |
| **Security-Info-Types** | `ParticipantSecurityInfo` (PID 0x1005), `EndpointSecurityInfo` (PID 0x1004), `security_algo_info::*` (5 Algorithm-Requirement-Types), `AlgorithmRequirements` | DDS-Security 1.2 §7.4.1.6 + §7.4.7.1.6 (Algorithm-Negotiation) | discovery, security-rtps | CONNECTED + SPEC-MANDATED | — |
| **Property-List** | `PropertyList`, `Property`, `BinaryProperty`, `MAX_PROPERTIES`, `MAX_PROPERTY_STRING_LEN` | DDSI-RTPS 2.5 §9.6.3.2 (PID_PROPERTY_LIST) | dcps, security-runtime | CONNECTED | — |

**Zusammenfassung:** 166/166 Public-Items klassifiziert in 21 Families.
- 59 CONNECTED (durch direkte ext-Refs)
- 107 SPEC-MANDATED Public-API oder VENDOR-EXTENSION (Wire-Format-
  Konstanten + Library-API für End-User-Custom-RTPS-Stack-Builders),
  alle mit doc-as-hook Decision.
- 0 DEAD nach Layer-2-Pass-2 (`ENCAPSULATION_CDR2_BE/LE` in dcps wired).

## 4 Wiring

### 4.1 Dependencies

```toml
zerodds-qos = { path = "../qos", default-features = false, features = ["alloc"] }
zerodds-foundation = { path = "../foundation", default-features = false, features = ["alloc"] }
zerodds-types = { path = "../types", default-features = false, features = ["alloc"] }
zerodds-cdr = { path = "../cdr", default-features = false, features = ["alloc"] }
zerodds-inspect-endpoint = { path = "../inspect-endpoint", optional = true }
```

### 4.2 Dependents

`zerodds-discovery`, `zerodds-dcps`, `zerodds-transport`, `zerodds-security-*`,
plus alle anderen Layer-2/3/4-Crates die Wire-Types benötigen.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | std + alloc + foundation/std |
| `alloc` | ✅ (via std) | Heap |
| `safety` | ❌ | reserved |
| `inspect` | ❌ | PDE-Tap-Hooks im HistoryCache (default OFF, R-034) |

## 5 Spec-Relevanz

- DDSI-RTPS 2.5: §8.3 (Wire-Format), §8.4 (Behavior-Modules), §8.5 (Discovery),
  §9 (PSM-Specifics)
- DDS-Security 1.2 §7.4 (Builtin-Endpoint-Slots-Wire-Format)
- XTypes 1.3 §7.4 (CDR-Encapsulation-Headers)
- DDS 1.4 §2.2.5 (Builtin-Topic-Data)

K3b-Spec-Audit-Status: 121 done / 0 partial / 0 open / 3 n/a.

## 6 Cleanup-Findings

Layer-2 Pass 1: 31 License-Header, 54 Phase-X-Marker rewriting,
3 Bulk-Strip-Doc-Comment-Bugs gefixt.

Layer-2 Pass 2 (Coherence-Audit):
- DEAD-Fix `ENCAPSULATION_CDR2_BE/LE`: dcps::strip_user_encap nutzt jetzt
  benannte Konstanten statt Magic-Bytes → CONNECTED.

## 7 Cleanup-Actions

Bereits abgeschlossen.

## 8 Spec-Doc-Updates

`docs/spec-coverage/ddsi-rtps-2.5.md` — alle Sektionen done.

## 9 Doc-Artefacts

- [x] Cargo.toml RC1
- [x] lib.rs-Header
- [x] README mit Module-Tabelle + Quick-Start

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-rtps                                  # ✅ 647 passed
cargo clippy -p zerodds-rtps --all-targets -- -D warnings   # ✅
cargo doc -p zerodds-rtps --no-deps                         # ✅ (4 minor pre-existing private-link warnings)
```

## 11 RC1-DoD-Checkliste

- [x] §1.1-§1.13 alle ✅

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
