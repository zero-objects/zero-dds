# DDSI-RTPS 2.5 — Spec-Coverage

**PDF:** `docs/standards/cache/omg/ddsi-rtps-2.5.pdf` (206 Seiten, OMG formal/2022-04-01)

Folgt dem Format aus `docs/spec-coverage/PROCESS.md`. Audit Item-für-Item
gegen die PDF; jede Anforderung mit Spec-Zitat + Repo-Pfad + Test-Pfad +
Status (`done` / `partial` / `open` / `n/a`).

**Kontext:** `crates/rtps/` ist live mit ~30 Files + 458 Tests
(datagram/header/header_extension/fragment_assembler/inline_qos/
message_builder/parameter_list/participant_data/publication_data/
subscription_data/qos_bridge/reader/reader_proxy/receiver_state/
reliable_reader/reliable_writer/submessage_header/submessages/
wire_types/writer/writer_proxy + endpoint_security_info/
property_list/security_algo_info). `crates/discovery/` ergaenzt
(spdp.rs + sedp/* + type_lookup/* + endpoint_match.rs +
capabilities.rs) mit 111 Tests. SPDP+SEDP+TypeLookup live + alle 16
Submessages + HeaderExtension + Reliable-Stack + Fragmentation.

---

## §1 Scope

### 1.1 DDS-Interop-Wire-Protocol

**Spec:** §1, S. 1 — "DDS Real-Time Publish-Subscribe Wire-Protocol."

**Repo:** `crates/rtps/src/lib.rs`.

**Tests:** Crate-weit.

**Status:** done

---

## §2 Conformance

### 2.1 Conformance gemaess §8.4.2 (Behavior Module)

**Spec:** §2.

**Repo:** Vollstaendige Behavior-Modul-Abdeckung:
- Stateless Best-Effort: `crates/discovery/src/spdp.rs::SpdpBeacon`
  + `crates/discovery/src/security/stateless.rs::StatelessMessageWriter`.
- Stateless Reliable: `crates/rtps/src/reliable_stateless_writer.rs::
  ReliableStatelessWriter` (T1-T12).
- Stateful Best-Effort: `crates/rtps/src/{writer,reader}.rs`.
- Stateful Reliable: `crates/rtps/src/{reliable_writer,reliable_reader}.rs`
  mit Fragmentation (`fragment_assembler.rs`).

**Tests:** Per Behavior-Variante eigene Test-Suite; E2E in
`tests/reliable_e2e.rs` (0%/10%/30% Packet-Loss Best-Effort+Reliable).

**Status:** done

---

## §3 Normative References

### 3.1 [DDS] DDS 1.4 / [IDL] IDL 4.2 / [DDS-XTYPES] XTypes 1.2

**Spec:** §3.

**Repo:** `crates/dcps/`, `crates/idl/`, `crates/types/`.

**Tests:** siehe pro Spec-Coverage-Doku.

**Status:** done

### 3.2 NTPv3, MD5, SCTP-CRC32c, AUTOSAR-CRC

**Spec:** §3 — normative References.

**Repo:** Alle 4 Standards referenziert + im Code:
- NTPv3-Timestamp-Format: `crates/rtps/src/header_extension.rs::HeTimestamp`
  + `crates/dcps/src/time.rs::Time` (sec+nanosec/fraction).
- MD5 (RFC 1321) als 128-bit-KeyHash + ChecksumValue::Md5 in
  `crates/rtps/src/header_extension.rs::ChecksumValue` + KeyHash-Pfad
  in `crates/cdr/src/keyhash.rs`.
- SCTP-CRC32c (Castagnoli, RFC 3309): `ChecksumKind::Crc32c` +
  ChecksumValue::Crc32c.
- AUTOSAR-CRC-64-XZ (ISO 14229): `ChecksumKind::Crc64` +
  ChecksumValue::Crc64.

**Tests:** `header_extension::tests::roundtrip_with_crc32c_le`,
`roundtrip_with_crc64_be`, `roundtrip_with_md5_le`,
`checksum_kind_roundtrip`, `checksum_kind_wire_size`,
`empty_he_le_roundtrip` (NTPv3 indirekt via HeTimestamp).

**Status:** done

---

## §4 Terms

### 4 Terminologie (CDR/DDS/EDP/GUID/PDP/PIM/PSM/RTPS/SEDP)

**Spec:** §4 — Glossar.

**Repo:** Begriffe konsistent verwendet.

**Tests:** —

**Status:** `n/a (informative)` — Glossar, keine normative
Anforderung.

---

## §5 Symbols

### 5 Symbols + Akronyme

**Spec:** §5.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Symbol-/Akronym-Liste.

---

## §6 Additional Information

### 6.1-6.4 Changes/How-to-Read/Acks/Maturity

**Spec:** §6.1-6.4.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Sections (Change-Log,
Lese-Hinweise, Acknowledgments, Maturity-Statement).

---

## §7 Overview / PIM/PSM Architektur

### 7.1 Introduction (Wire-Protocol-Anforderungen)

**Spec:** §7.1, S. 7.

**Repo:** n/a

**Tests:** n/a

**Status:** done

### 7.2 Requirements for a DDS Wire-protocol

**Spec:** §7.2, S. 7-8.

**Repo:** n/a — Anforderungen erfuellt durch Crate-Architektur.

**Tests:** Crate-weit.

**Status:** done

### 7.3 The RTPS Wire-protocol

**Spec:** §7.3, S. 8.

**Repo:** Crate `crates/rtps/`.

**Tests:** Wire-Tests.

**Status:** done

### 7.4.1 Structure Module (PIM)

**Spec:** §7.4.1, S. 9-11 — GUID/Cache/Endpoint/Participant.

**Repo:** `wire_types.rs` mit GUID/SequenceNumber/Locator/etc.

**Tests:** wire-types Tests.

**Status:** done

### 7.4.2 Messages Module (PIM)

**Spec:** §7.4.2.

**Repo:** `submessages/` Modul (alle 16 Submessages) +
`message_builder.rs`.

**Tests:** Submessage-Roundtrip-Tests.

**Status:** done

### 7.4.3 Behavior Module (PIM)

**Spec:** §7.4.3.

**Repo:** Vier kanonische Behavior-Implementations live:
StatelessBE (`spdp.rs`), StatelessReliable
(`reliable_stateless_writer.rs`), StatefulBE (`writer.rs`/`reader.rs`),
StatefulReliable (`reliable_writer.rs`/`reliable_reader.rs`).
Jede mit eigenem State-Machine-Modul.

**Tests:** Pro Variante eigene Test-Suite (siehe §2.1).

**Status:** done

### 7.4.4 Discovery Module (PIM)

**Spec:** §7.4.4.

**Repo:** `crates/discovery/src/spdp.rs` + `sedp/`.

**Tests:** SPDP+SEDP-Tests.

**Status:** done

### 7.5 RTPS Platform Specific Model (PSM)

**Spec:** §7.5, S. 11 — UDPv4-PSM ist Pflicht.

**Repo:** UDPv4-PSM via `crates/transport-udp/`.

**Tests:** Transport-Tests.

**Status:** done

### 7.6 RTPS Transport Model

**Spec:** §7.6, S. 11 — 16-Byte-Adresse, 4-Byte-Port, Datagram,
Drop-on-Corrupt.

**Repo:** `wire_types.rs::Locator` (16+4) + UDP-Datagram-Pfad.

**Tests:** Locator-Tests.

**Status:** done

---

## §8.2 Structure Module

### 8.2.1.2 GUID_t (16 Octets)

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::Guid`.

**Tests:** GUID-Tests.

**Status:** done

### 8.2.1.2 GuidPrefix_t (12 Octets) + GUIDPREFIX_UNKNOWN

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::GuidPrefix`.

**Tests:** —

**Status:** done

### 8.2.1.2 EntityId_t (4 Octets) + ENTITYID_UNKNOWN

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::EntityId`.

**Tests:** —

**Status:** done

### 8.2.1.2 SequenceNumber_t (64-bit) + SEQUENCENUMBER_UNKNOWN

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::SequenceNumber` (i64-Repr).

**Tests:** —

**Status:** done

### 8.2.1.2 Locator_t (kind+port+16-byte-addr) + 5 Reserved-Konstanten

**Spec:** §8.2.1.2 — LOCATOR_INVALID/KIND_INVALID/RESERVED/UDPv4/UDPv6.

**Repo:** `crates/rtps/src/wire_types.rs::Locator` mit Konstanten
INVALID/RESERVED/UDP_V4_ANY/UDP_V6_ANY/SHM_ANY + PORT_INVALID +
ADDRESS_INVALID + Konstruktoren udp_v4/udp_v6/tcp_v4/uds/shm.

**Tests:** `wire_types::tests::locator_invalid_constant_matches_spec`,
`locator_reserved_constant_kind_is_zero`,
`locator_udp_v4_any_kind_is_one`,
`locator_udp_v6_any_kind_is_two`,
`locator_shm_any_uses_vendor_kind`,
`locator_udp_v6_constructor_keeps_full_address`,
`locator_udp_v6_roundtrip_le`,
`locator_udp_v4_layout`, `locator_uds_layout`,
`locator_kind_roundtrip`, `locator_kind_rejects_unknown` (Negativ),
`locator_invalid_kind_decoded` (Negativ).

**Status:** done

### 8.2.1.2 TopicKind_t (NO_KEY/WITH_KEY)

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::TopicKind`.

**Tests:** —

**Status:** done

### 8.2.1.2 ChangeKind_t (ALIVE/ALIVE_FILTERED/NOT_ALIVE_DISPOSED/NOT_ALIVE_UNREGISTERED)

**Spec:** §8.2.1.2.

**Repo:** `crates/rtps/src/history_cache.rs::ChangeKind` mit allen
4 Spec-Varianten + `NotAliveDisposedUnregistered` (kombinierter Marker)
+ `is_relevant()` (false fuer AliveFiltered) + `is_alive_kind()`
(true fuer Alive + AliveFiltered).

**Tests:** `history_cache::tests::change_kind_alive_is_relevant_and_alive`,
`change_kind_alive_filtered_is_alive_but_not_relevant`,
`change_kind_not_alive_kinds_are_not_alive`,
`change_kind_distinct_variants`.

**Status:** done

### 8.2.1.2 ReliabilityKind_t (BEST_EFFORT/RELIABLE)

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::ReliabilityKind`.

**Tests:** —

**Status:** done

### 8.2.1.2 InstanceHandle_t

**Spec:** §8.2.1.2.

**Repo:** Indirekt via DCPS-`InstanceHandle`.

**Tests:** —

**Status:** done

### 8.2.1.2 ProtocolVersion_t mit Aliases (1.0/1.1/2.0/2.1/2.2/2.4 + 2.5)

**Spec:** §8.2.1.2.

**Repo:** `crates/rtps/src/wire_types.rs::ProtocolVersion` mit allen
Aliases V1_0/V1_1/V2_0/V2_1/V2_2/V2_3/V2_4/V2_5 + `CURRENT` Alias
+ Default V2_5.

**Tests:** `wire_types::tests::protocol_version_aliases_match_spec`,
`protocol_version_all_aliases_roundtrip`,
`protocol_version_default_is_2_5`,
`protocol_version_roundtrip`.

**Status:** done

### 8.2.1.2 VendorId_t (OMG-verwaltete Liste) + VENDORID_UNKNOWN

**Spec:** §8.2.1.2.

**Repo:** `header.rs::VendorId` + Konstanten.

**Tests:** —

**Status:** done

### 8.2.2 HistoryCache (add/remove/get_seq_min/get_seq_max)

**Spec:** §8.2.2.

**Repo:** `crates/rtps/src/history_cache.rs` (WHC/RHC).

**Tests:** History-Cache-Tests.

**Status:** done

### 8.2.3 CacheChange (kind/writerGuid/instanceHandle/sequenceNumber/data_value/inlineQos)

**Spec:** §8.2.3.

**Repo:** `wire_types.rs::CacheChange`.

**Tests:** —

**Status:** done

### 8.2.4 RTPS Entity (GUID = prefix + entityId; ENTITYID_PARTICIPANT)

**Spec:** §8.2.4.

**Repo:** `wire_types.rs`.

**Tests:** —

**Status:** done

### 8.2.5 RTPS Participant (defaultUni/MulticastLocatorList + protocolVersion + vendorId + guidPrefix)

**Spec:** §8.2.5.

**Repo:** `participant_data.rs`.

**Tests:** —

**Status:** done

### 8.2.7 RTPS Endpoint (uni/multicastLocatorList + reliabilityLevel + topicKind + endpointGroup)

**Spec:** §8.2.7.

**Repo:** Endpoint-Felder verteilt auf `publication_data.rs` /
`subscription_data.rs`.

**Tests:** —

**Status:** done

### 8.2.8 RTPS Writer (Attribute aus §8.4.7.1)

**Spec:** §8.2.8.

**Repo:** `writer.rs`+`reliable_writer.rs`.

**Tests:** Writer-Tests.

**Status:** done

### 8.2.9 RTPS Reader (Attribute aus §8.4.10)

**Spec:** §8.2.9.

**Repo:** `reader.rs`+`reliable_reader.rs`.

**Tests:** Reader-Tests.

**Status:** done

### 8.2.10 DDS DataWriter/Reader-Bindung (T1-T5 Transitions)

**Spec:** §8.2.10.

**Repo:** DCPS-Layer (`crates/dcps/`).

**Tests:** DCPS-Tests.

**Status:** done

---

## §8.3 Messages Module

### 8.3.2 SubmessageKind (16 Types)

**Spec:** §8.3.2 — RTPS_HE/DATA/HEARTBEAT/ACKNACK/PAD/INFO_TS/
INFO_REPLY/INFO_DST/INFO_SRC/DATA_FRAG/NACK_FRAG/HEARTBEAT_FRAG/
GAP/INFO_REPLY_IP4/SEC_*

**Repo:** `submessage_header.rs::SubmessageId`.

**Tests:** SubmessageId-Tests.

**Status:** done

### 8.3.2 Time_t mit Nano-Resolution (TIME_ZERO/INVALID/INFINITE)

**Spec:** §8.3.2.

**Repo:** `crates/dcps/src/time.rs::Time` (DCPS+RTPS-shared) mit
`ZERO` / `INVALID` / `INFINITE` Sentinels + `is_zero()`/`is_invalid()`/
`is_infinite()` Predicates. RTPS-Submessage-Body nutzt
`crates/rtps/src/header_extension.rs::HeTimestamp` (sec+fraction).

**Tests:** `zerodds_dcps::time::tests::time_zero_sentinel`,
`time_invalid_sentinel`, `time_infinite_sentinel`,
`get_current_time_is_recent` (positive),
`duration_to_core_maps_infinite_to_max`,
`duration_negative_sec_maps_to_zero` (Negativ).

**Status:** done

### 8.3.2 Checksum_t (CHECKSUM_INVALID)

**Spec:** §8.3.2.

**Repo:** `header_extension.rs::ChecksumKind::None`/`ChecksumValue`.

**Tests:** Checksum-Tests.

**Status:** done

### 8.3.2 MessageLength_t (MESSAGE_LENGTH_INVALID)

**Spec:** §8.3.2.

**Repo:** `header_extension.rs` mit `Option<u32>` + None=invalid.

**Tests:** —

**Status:** done

### 8.3.2 GroupDigest_t

**Spec:** §8.3.2 / §8.3.5.10 — 16-byte CDR-encoded 128-bit-Digest
fuer Group-Membership.

**Repo:** `crates/rtps/src/group_digest.rs::GroupDigest` mit
`UNKNOWN`-Sentinel (16 Null-Bytes), `from_prefixes(prefixes)` (sortiert
GuidPrefixes lexikographisch und MD5-hashed via `md-5`-Crate, byte-
identisch zu Cyclone DDS), `to_bytes` / `read_from(bytes)` Wire-
Encoding.

**Tests:** `group_digest::tests::md5_empty_input_matches_rfc1321_test_vector`,
`md5_abc_matches_rfc1321_test_vector`,
`md5_message_digest_matches_rfc1321_test_vector`,
`md5_long_input_matches_rfc1321_test_vector` (2-Block-Pfad),
`group_digest_unknown_is_zero`,
`group_digest_from_empty_prefixes_is_md5_of_empty`,
`group_digest_independent_of_input_order`,
`group_digest_distinguishes_different_groups`,
`group_digest_roundtrip`,
`group_digest_read_from_truncated_rejects` (Negativ),
`group_digest_handles_more_than_5_prefixes_two_block_md5`.

**Status:** done

### 8.3.2 UExtension4_t / WExtension8_t

**Spec:** §8.3.2.

**Repo:** `crates/rtps/src/wire_types.rs::UExtension4` (4-byte vendor-
opaque) + `WExtension8` (8-byte vendor-opaque) mit u32/u64-BE
Konvertern + roundtrip-Identitaet. HE-Submessage nutzt UExtension4
ueber `header_extension.rs::HeaderExtension.uextension4`.

**Tests:** `wire_types::tests::uextension4_roundtrip`,
`uextension4_u32_be_roundtrip`,
`uextension4_default_is_zero`,
`wextension8_roundtrip`,
`wextension8_u64_be_roundtrip`,
`wextension8_default_is_zero`.

**Status:** done

### 8.3.3 Message = Header + 1..* Submessages; HeaderExtension optional direkt nach Header

**Spec:** §8.3.3.

**Repo:** `datagram.rs` + `header_extension.rs`.

**Tests:** Message-Roundtrip-Tests.

**Status:** done

### 8.3.3.1 Header: protocol(4) + version(2) + vendorId(2) + guidPrefix(12) = 20 Byte

**Spec:** §8.3.3.1.

**Repo:** `header.rs::Header`.

**Tests:** Header-Tests.

**Status:** done

### 8.3.3.1.1 protocol == PROTOCOL_RTPS

**Spec:** §8.3.3.1.1.

**Repo:** `header.rs` Magic-Check.

**Tests:** —

**Status:** done

### 8.3.3.1.2 version = 2.5

**Spec:** §8.3.3.1.2.

**Repo:** `crates/rtps/src/wire_types.rs::ProtocolVersion::V2_5` +
`Default::default()` liefert V2_5; `CURRENT` Alias fuer den
aktuellsten unterstuetzten Spec-Stand.

**Tests:** `wire_types::tests::protocol_version_default_is_2_5`,
`protocol_version_aliases_match_spec`,
`protocol_version_all_aliases_roundtrip`.

**Status:** done

### 8.3.3.2 HeaderExtension Submessage (messageLength/rtpsSendTimestamp/uExtension4/wExtension8/messageChecksum/parameters + 7 Flags)

**Spec:** §8.3.3.2.

**Repo:** `crates/rtps/src/header_extension.rs::HeaderExtension` mit
allen Feldern (messageLength/timestamp/uextension4/extension16/
checksum/parameters) + allen 7 Flags (E/L/W/U/V/C/P; C=2-bit fuer
None/CRC32C/CRC64/MD5). `flag_byte` baut Flag-Byte aus gesetzten
Optional-Feldern.

**Tests:** `header_extension::tests::empty_he_le_roundtrip`,
`empty_he_be_roundtrip`,
`flag_byte_empty_le`,
`flag_byte_with_message_length`,
`flag_byte_all_optional_set_le`,
`flag_byte_includes_all_seven_logical_flags`,
`roundtrip_with_message_length_only_le`,
`roundtrip_with_timestamp_le`,
`roundtrip_with_uextension4_be`,
`roundtrip_with_extension16_le`,
`roundtrip_with_crc32c_le`,
`roundtrip_with_crc64_be`,
`roundtrip_with_md5_le`,
`roundtrip_with_parameter_list`,
`roundtrip_all_fields_set_le`,
`roundtrip_all_fields_set_be`.

**Status:** done

### 8.3.3.3 SubmessageHeader: submessageId(1) + flags(1) + submessageLength(2)

**Spec:** §8.3.3.3.

**Repo:** `submessage_header.rs`.

**Tests:** —

**Status:** done

### 8.3.4 Receiver-State (sourceVersion/VendorId/GuidPrefix/destGuidPrefix/uni-/multicastReplyLocatorList/haveTimestamp/timestamp/messageLength/messageChecksum/rtpsSend-/ReceptionTimestamp/clockSkewDetected/parameters)

**Spec:** §8.3.4.

**Repo:** `receiver_state.rs::ReceiverState`.

**Tests:** Receiver-Tests.

**Status:** done — alle Felder vorhanden inkl. message_length+
messageChecksum.

### 8.3.4.1 Receiver-Rules (1-6: Header-invalid/skip-Logik/unknown-Sub/X-Flags-ignore/etc.)

**Spec:** §8.3.4.1.

**Repo:** Skip-Logik in `datagram.rs`/`submessage_header.rs`.

**Tests:** —

**Status:** done

### 8.3.5 SubmessageElements (alle 19)

**Spec:** §8.3.5.1-19.

**Repo:** `crates/rtps/src/wire_types.rs` + `submessages.rs`. Alle
Spec-Elemente: GuidPrefix, EntityId, VendorId, ProtocolVersion (alle
8 Versionen), SequenceNumber, SequenceNumberSet, FragmentNumber,
FragmentNumberSet, Timestamp (HeTimestamp + DCPS Time), ParameterList,
Count (u32 in Heartbeat/AckNack/NackFrag), LocatorList,
SerializedData, SerializedDataFragment, ChangeCount, Checksum
(ChecksumValue), MessageLength, UExtension4, WExtension8,
GroupDigest_t.

**Tests:** Pro Element ein Roundtrip-Test:
`wire_types::tests::guid_prefix_roundtrip`,
`vendor_id_roundtrip`,
`protocol_version_all_aliases_roundtrip`,
`entity_id_roundtrip`,
`sequence_number_roundtrip_le`/`_be`,
`fragment_number_roundtrip_le_be`,
`locator_roundtrip_le`,
`uextension4_roundtrip`,
`wextension8_roundtrip`,
`uextension4_u32_be_roundtrip`,
`wextension8_u64_be_roundtrip`.
ParameterList/SequenceNumberSet/FragmentNumberSet/SerializedData(_Fragment)
in `parameter_list.rs::tests` + `submessages::tests`.
Count: implizit in Heartbeat/AckNack-Roundtrip-Tests in
`submessages::tests`.
GroupDigest_t in `crates/rtps/src/group_digest.rs::tests`
(11 Tests, RFC-1321-Vektoren + Order-Independenz +
Multi-Block-Pfad).

**Status:** done

---

## §8.3.6 Header Validity

### 8.3.6.3 Header invalid: <required octets / protocol != PROTOCOL_RTPS / major-version > supported

**Spec:** §8.3.6.3.

**Repo:** Validation in `datagram.rs`.

**Tests:** Header-Validity-Tests.

**Status:** done

### 8.3.6.4 Receiver-Update aus Header

**Spec:** §8.3.6.4.

**Repo:** `receiver_state.rs::on_header`.

**Tests:** —

**Status:** done

---

## §8.3.7 HeaderExtension Validity

### 8.3.7.3 HeaderExtension invalid (nicht direkt nach Header / Length-Mismatch / parameters malformed)

**Spec:** §8.3.7.3.

**Repo:** `crates/rtps/src/datagram.rs::decode_datagram` rejected HE,
das nicht erste Submessage nach RTPS-Header ist
(`HeaderExtension must appear directly after the RTPS header`).
Length-Mismatch + truncated-Body durch `decode_body` /
`validate_message_length`. Malformed ParameterList wird durch
`ParameterList::from_bytes` propagiert.

**Tests:** `datagram::tests::decode_rejects_header_extension_after_data_submessage` (Negativ),
`header_extension::tests::decode_rejects_truncated_header` (Negativ),
`decode_rejects_truncated_body_before_message_length` (Negativ),
`decode_rejects_oversized_body` (Negativ),
`decode_rejects_trailing_bytes_without_p_flag` (Negativ),
`decode_rejects_truncated_uextension4_body` (Negativ),
`decode_rejects_truncated_extension16_body` (Negativ),
`decode_rejects_truncated_md5_checksum_body` (Negativ),
`decode_rejects_malformed_parameter_list_when_p_flag_set` (Negativ),
`validate_message_length_mismatch` (Negativ).

**Status:** done

### 8.3.7.4 HeaderExtension-Updates fuer Receiver (messageLength/timestamps/clockSkew/extensions/checksum/parameters)

**Spec:** §8.3.7.4.

**Repo:** `crates/rtps/src/receiver_state.rs::ReceiverState::
apply_header_extension` setzt aus dem HE: `message_length` (L-Flag),
`timestamp` + `have_timestamp` (W-Flag = rtpsSendTimestamp),
`message_checksum` (C-Flags), `parameters` (P-Flag).
`note_clock_skew(now, threshold)` setzt das `clock_skew_detected`-Flag
wenn `|timestamp.seconds - now_seconds| > threshold`. uextension4 +
wextension8 sind in `HeaderExtension` selbst gespeichert; der
ReceiverState bietet via `parameters` den Pass-through.

**Tests:** `receiver_state::tests::apply_header_extension_updates_fields`
(messageLength + timestamp + checksum),
`apply_header_extension_with_parameters_sets_pl`,
`note_clock_skew_skipped_without_timestamp` (Negativ),
`note_clock_skew_within_threshold_does_not_flag` (Negativ),
`note_clock_skew_above_threshold_flags`.

**Status:** done

---

## §8.3.8 Submessages

### 8.3.8.1 AckNack

**Spec:** §8.3.8.1.

**Repo:** `submessages/acknack.rs`.

**Tests:** AckNack-Tests.

**Status:** done

### 8.3.8.2 Data

**Spec:** §8.3.8.2 — InlineQos/Data/Key/NonStandardPayload-Flags.

**Repo:** `crates/rtps/src/submessages.rs::DataSubmessage` mit
explicit `key_flag` + `non_standard_flag` (Spec §8.3.8.2 Tab. 8.43).
`write_body` setzt D/Q/K/N-Flag-Bits nach den Feldern;
`read_body_with_flags` extrahiert sie aus dem Submessage-Header
(siehe `crates/rtps/src/datagram.rs::decode_datagram`).

**Tests:** `submessages::tests::data_submessage_roundtrip_le`,
`data_submessage_roundtrip_be_with_empty_payload`,
`data_submessage_octets_to_inline_qos_is_16`,
`data_submessage_decode_rejects_truncated` (Negativ),
`data_submessage_key_flag_roundtrip`,
`data_submessage_non_standard_flag_roundtrip`,
`data_submessage_all_flags_combined_roundtrip` (alle 5 E/Q/D/K/N).

**Status:** done

### 8.3.8.3 DataFrag (Re-Assembly)

**Spec:** §8.3.8.3.

**Repo:** `submessages/data_frag.rs` + `fragment_assembler.rs`.

**Tests:** Frag-Tests.

**Status:** done

### 8.3.8.4 Gap (gapStart + gapList + GroupInfoFlag)

**Spec:** §8.3.8.4.

**Repo:** `crates/rtps/src/submessages.rs::GapSubmessage` mit
gapStart + gapList + optional `filteredCount` (K-Flag) und
GroupInfo-Sektion (G-Flag, Spec §8.3.8.4.2).

**Tests:** `submessages::tests::gap_submessage_roundtrip_le`,
`gap_decode_rejects_truncated_header` (Negativ),
`gap_with_filtered_count_roundtrip_le`,
`gap_with_group_info_roundtrip_be`,
`gap_with_group_info_and_filtered_count_combined` (beide Flags),
`gap_filtered_count_zero_is_distinct_from_none`,
`gap_decode_rejects_truncated_filtered_count` (Negativ).

**Status:** done

### 8.3.8.5 HeaderExtension (siehe §8.3.7)

**Spec:** §8.3.8.5 — Verweis auf die Submessage-Definition in §8.3.7.

**Repo:** Identisch zu §8.3.3.2 / §8.3.7.3 / §8.3.7.4 — siehe die
dortigen Items.

**Tests:** Siehe §8.3.3.2 / §8.3.7.3 / §8.3.7.4.

**Status:** done

### 8.3.8.6 Heartbeat (firstSN/lastSN/count + Final/Liveliness/GroupInfo Flags)

**Spec:** §8.3.8.6.

**Repo:** `crates/rtps/src/submessages.rs::HeartbeatSubmessage` mit
firstSN + lastSN + count + Final-Flag + Liveliness-Flag + GroupInfo-
Flag (alle 3 Submessage-Flags), optional GroupInfo-Sektion mit
writerSet + secureWriterSet (Spec §8.3.8.6.1).

**Tests:** `submessages::tests::heartbeat_submessage_roundtrip_le`,
`heartbeat_submessage_no_final_flag_when_disabled` (Negativ),
`heartbeat_submessage_liveliness_flag_roundtrip`,
`heartbeat_decode_rejects_truncated` (Negativ),
`heartbeat_with_empty_group_info_roundtrip_le`,
`heartbeat_with_writer_set_roundtrip_be`.

**Status:** done

### 8.3.8.7 HeartbeatFrag

**Spec:** §8.3.8.7.

**Repo:** `submessages/heartbeat_frag.rs`.

**Tests:** —

**Status:** done

### 8.3.8.8 InfoDestination

**Spec:** §8.3.8.8.

**Repo:** `submessages.rs::InfoDstSubmessage`.

**Tests:** —

**Status:** done

### 8.3.8.9 InfoReply

**Spec:** §8.3.8.9.

**Repo:** `crates/rtps/src/submessages.rs::InfoReplySubmessage` mit
unicast_locators + optional multicast_locators (M-Flag);
`crates/rtps/src/datagram.rs::decode_datagram` matched
`SubmessageId::InfoReply`. `crates/rtps/src/receiver_state.rs::
ReceiverState::apply_info_reply` propagiert die Locator-Listen
in den Reply-Pfad.

**Tests:** `submessages::tests::info_reply_unicast_only_roundtrip_le`,
`info_reply_with_multicast_roundtrip_le`,
`info_reply_with_multicast_roundtrip_be`,
`info_reply_empty_unicast_list_is_valid`,
`info_reply_decode_rejects_oversized_locator_list_length` (Negativ),
`info_reply_decode_rejects_truncated_length_field` (Negativ).

**Status:** done

### 8.3.8.10 InfoSource

**Spec:** §8.3.8.10.

**Repo:** `crates/rtps/src/submessages.rs::InfoSourceSubmessage` mit
allen 4 Wire-Feldern (unused / protocolVersion / vendorId /
guidPrefix). `crates/rtps/src/receiver_state.rs::ReceiverState::
apply_info_source` ueberschreibt source_version + source_vendor_id
+ source_guid_prefix UND **resetted** Reply-Locator-Listen + setzt
`have_timestamp = false` (Spec §8.3.8.9.4 Schritt 4).

**Tests:** `submessages::tests::info_source_roundtrip_le`,
`info_source_roundtrip_be`,
`info_source_wire_size_is_20`,
`info_source_decode_rejects_truncated` (Negativ),
`info_source_unused_field_roundtrips`.
`receiver_state::tests::apply_info_source_resets_reply_locators_and_timestamp`.

**Status:** done

### 8.3.8.11 InfoTimestamp

**Spec:** §8.3.8.11.

**Repo:** `crates/rtps/src/submessages.rs::InfoTimestampSubmessage`
(neu in K3b-B) mit Time_t-Body + I-Flag (`INFO_TIMESTAMP_FLAG_INVALIDATE`).
`crates/rtps/src/datagram.rs::decode_datagram` matched
`SubmessageId::InfoTs`. `crates/rtps/src/receiver_state.rs::
ReceiverState::apply_info_timestamp` setzt `timestamp` +
`have_timestamp = true`; bei `invalidate=true` wird
`have_timestamp = false`. Skew-Korrektur ueber
`note_clock_skew(now_seconds, threshold)` setzt
`clock_skew_detected`-Flag (Spec §8.3.4.1 Receiver-Rule 4).

**Tests:** `submessages::tests::info_timestamp_roundtrip_le`,
`info_timestamp_roundtrip_be`,
`info_timestamp_invalidate_flag_yields_empty_body`,
`info_timestamp_decode_rejects_truncated_when_no_invalidate` (Negativ),
`info_timestamp_decode_with_invalidate_ignores_body`.
`receiver_state::tests::apply_info_timestamp_sets_value`,
`apply_info_timestamp_with_invalidate_clears`,
`note_clock_skew_skipped_without_timestamp` (Negativ),
`note_clock_skew_within_threshold_does_not_flag` (Negativ),
`note_clock_skew_above_threshold_flags`.

**Status:** done

### 8.3.8.12 NackFrag

**Spec:** §8.3.8.12.

**Repo:** `submessages/nack_frag.rs`.

**Tests:** —

**Status:** done

### 8.3.8.13 Pad

**Spec:** §8.3.8.13.

**Repo:** `submessages.rs::PadSubmessage`.

**Tests:** —

**Status:** done

---

## §8.4 Behavior Module

### 8.4.2.1 Comm-Pfad (Messages, Receiver-Rules, Timing-Konfigurabel, SPDP+SEDP)

**Spec:** §8.4.2.1.

**Repo:** Verteilt auf `datagram.rs`+`writer.rs`+`reader.rs`+
`crates/discovery/src/{spdp,sedp/}`.

**Tests:** —

**Status:** done

### 8.4.2.2 Writer-Verhalten (No Out-of-Order; Heartbeats; Inline-QoS; NACK-Antworten)

**Spec:** §8.4.2.2.

**Repo:** `crates/rtps/src/writer.rs` (Best-Effort) +
`reliable_writer.rs` (Reliable StatefulWriter mit handle_acknack +
periodischem Heartbeat-Tick + Inline-QoS-Pfad ueber
`DataSubmessage.inline_qos`). Writer sendet streng monoton steigende
SequenceNumbers (No-Out-of-Order); HEARTBEAT enthaelt firstSN/lastSN/
count + Final/Liveliness/GroupInfo-Flags (Spec §8.3.8.6).
Group-Info-Sektion mit writerSet + secureWriterSet ueber
`HeartbeatSubmessage.group_info`-Felder.

**Tests:** `reliable_writer::tests` (39 Tests inkl. handle_acknack-
Variantenpfade + retransmits + heartbeat-tick + group-info);
`writer::tests` (Best-Effort-Pfad);
`inline_qos::tests::reply_inline_qos_writer_side_serialisation`.

**Status:** done

### 8.4.2.3 Reader-Verhalten (ACKNACK auf HEARTBEAT; once-acked-always-acked)

**Spec:** §8.4.2.3.

**Repo:** `crates/rtps/src/reliable_reader.rs::ReliableReader` mit
periodischem ACKNACK-Tick, Pre-Emptive-ACKNACK beim Add-Writer-Proxy
(Spec §8.4.12.5, "if the Reader received no DATA or HEARTBEAT yet
it sends a preemptive ACKNACK"), `count_modular`-Counter (wraps
u32), `highest_received_sn`-Once-Acked-Buchhaltung im
`writer_proxy`. `crates/rtps/src/reader.rs` Best-Effort-Pfad.

**Tests:** `reliable_reader::tests::pre_emptive_acknack_emitted_after_add_writer_proxy`,
`no_pre_emptive_acknack_without_proxy` (Negativ),
`initial_proxy_from_config_does_not_send_pre_emptive` (Negativ),
`pre_emptive_acknack_carries_info_dst`. Plus 30+ weitere Tests fuer
ACKNACK-Generation, Heartbeat-Reaction, Once-Acked-Persistenz.

**Status:** done

### 8.4.3 Stateless+Stateful Reference-Implementations

**Spec:** §8.4.3.

**Repo:**
- Stateful Best-Effort + Reliable: `crates/rtps/src/{writer,reliable_writer,reader,reliable_reader}.rs`.
- Stateless Best-Effort: `crates/discovery/src/spdp.rs::SpdpBeacon`
  (Multicast-Beacon, kein Reader-Proxy-State) +
  `crates/discovery/src/security/stateless.rs::StatelessMessageWriter`
  (Multi-Reader-Fan-out fuer
  `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`, kein Cache, kein
  Resend, kein HEARTBEAT).
- Stateless Reliable: `crates/rtps/src/reliable_stateless_writer.rs::
  ReliableStatelessWriter` (siehe §8.4.8.2).

**Tests:** `spdp::tests`, `security::stateless::tests` (Stateless);
`reliable_writer::tests`, `reliable_reader::tests` (Stateful);
`reliable_stateless_writer::tests` (Stateless-Reliable).

**Status:** done

### 8.4.4 Reliability-Kombinationen

**Spec:** §8.4.4.

**Repo:** `endpoint_match.rs` (in `discovery/`).

**Tests:** —

**Status:** done

### 8.4.6 Types: Duration_t, Change*StatusKind, InstanceHandle_t, ParticipantMessageData

**Spec:** §8.4.6.

**Repo:** Duration_t in `crates/qos/src/duration.rs::Duration` (DDS-PSM)
+ `crates/rtps/src/header_extension.rs::HeTimestamp` (RTPS-Wire);
ChangeKind/Change*StatusKind in
`crates/rtps/src/history_cache.rs::ChangeKind` mit allen 5
Spec-Varianten + `is_relevant`/`is_alive_kind`-Predicates;
InstanceHandle_t in `crates/dcps/src/instance_handle.rs::InstanceHandle`;
ParticipantMessageData in
`crates/rtps/src/participant_message_data.rs::ParticipantMessageData`
mit AUTOMATIC + MANUAL_BY_PARTICIPANT + ZeroDDS-Vendor-Kind.

**Tests:** `qos::duration::tests::*`,
`history_cache::tests::change_kind_*`,
`participant_message_data::tests::*`.

**Status:** done

### 8.4.7.1 Writer-Attrs (pushMode/heartbeatPeriod/nackResponseDelay/etc.)

**Spec:** §8.4.7.1.

**Repo:** `writer.rs`.

**Tests:** —

**Status:** done

### 8.4.7.1.1 Default-Timing (nackResponseDelay=200ms, nackSuppressionDuration=0)

**Spec:** §8.4.7.1.1.

**Repo:** Konstanten in `writer.rs`.

**Tests:** —

**Status:** done

### 8.4.7.1.3 new_change(kind/data/inlineQos/handle)

**Spec:** §8.4.7.1.3.

**Repo:** `writer.rs::new_change`.

**Tests:** —

**Status:** done

### 8.4.7.2 StatelessWriter

**Spec:** §8.4.7.2.

**Repo:** Best-Effort-Stateless in `crates/discovery/src/spdp.rs::
SpdpBeacon` (SPDP-Multicast) +
`crates/discovery/src/security/stateless.rs::StatelessMessageWriter`
(DDS-Security stateless message). Reliable-Stateless in
`crates/rtps/src/reliable_stateless_writer.rs::ReliableStatelessWriter`
(siehe §8.4.8.2).

**Tests:** `spdp::tests::*`,
`security::stateless::tests::*`,
`reliable_stateless_writer::tests::*`.

**Status:** done

### 8.4.7.3 ReaderLocator (highestSentChangeSN/requestedChanges/locator/expectsInlineQos + Ops)

**Spec:** §8.4.7.3.

**Repo:** `crates/rtps/src/reader_proxy.rs::ReaderProxy` (Stateful) +
`crates/discovery/src/security/stateless.rs::StatelessMessageWriter::
reader_proxies` (Stateless-Vector). ReaderLocator-aequivalente Felder
in ReaderProxy: `remote_reader_guid`, `unicast_locators`,
`multicast_locators`, `expects_inline_qos`, `highest_sent_change_sn`,
`requested_changes`, `acked_changes`. Ops: `add_proxy`/`remove_proxy`/
`update_locators`. Stateless-ReaderLocator-Pool ueber
`ReliableStatelessWriter::set_locators`.

**Tests:** `reader_proxy::tests::*` (12 Tests fuer alle Ops),
`reliable_stateless_writer::tests::set_locators_t11_replaces_list`,
`security::stateless::tests::add_remove_reader_proxy_idempotent`.

**Status:** done

### 8.4.7.4 StatefulWriter (matched_readers + is_acked_by_all)

**Spec:** §8.4.7.4.

**Repo:** `reliable_writer.rs`.

**Tests:** —

**Status:** done

### 8.4.7.5 ReaderProxy (alle Felder + Ops)

**Spec:** §8.4.7.5.

**Repo:** `reader_proxy.rs`.

**Tests:** —

**Status:** done

### 8.4.8.1 Best-Effort StatelessWriter (T1-T5)

**Spec:** §8.4.8.1.

**Repo:** SPDP-Sender (Multicast-Best-Effort).

**Tests:** —

**Status:** done

### 8.4.8.2 Reliable StatelessWriter (T1-T12)

**Spec:** §8.4.8.2 — Reliable-Stateless Writer-State-Machine mit 12
Transitions (Tab. 8.46).

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs::
ReliableStatelessWriter` mit allen 12 Transitions:
- T1 `new_change(kind, payload)` → SequenceNumber.
- T2/T3 `tick(now)` → DATA-Burst + periodischer HEARTBEAT.
- T4/T5 `handle_acknack(ack)` → `lowest_unacked` advance, requested-
  Pool fuellen.
- T6 `tick` (NACK-Drain) → Retransmits aus dem Pool senden.
- T7 `is_acked_to(sn)` → once-acked-always-acked Predicate.
- T8 `purge_acked()` → Cache-LowWater-Cleanup.
- T9 KeepLast in `HistoryCache::insert` (existing).
- T10 `shutdown()` → Cache + Retransmit-Pool leeren.
- T11 `set_locators(locators)` → Reply-Locator-List Update.
- T12 `stats()` → Diagnose-Snapshot.

`max_per_tick`-Cap (default 16) als DoS-Schutz gegen ueberbordende
ACKNACK-Anfragen. `heartbeat_count` wraps u32 (siehe §8.4.15.7).

**Tests:** `reliable_stateless_writer::tests`:
`new_change_assigns_monotonic_sn_t1`,
`handle_acknack_advances_lowest_unacked_t4`,
`handle_acknack_only_advances_t4_once_acked_always_acked`,
`handle_acknack_with_requested_bits_queues_retransmits_t6`,
`is_acked_to_t7`,
`purge_acked_t8_removes_acked_samples`,
`tick_emits_heartbeat_t3`,
`tick_does_not_emit_heartbeat_when_cache_empty` (Negativ),
`tick_emits_retransmits_for_requested_sns_t6`,
`tick_caps_retransmits_at_max_per_tick`,
`shutdown_clears_state_t10`,
`set_locators_t11_replaces_list`,
`heartbeat_count_wraps_at_u32_max_t3_modular`,
`stats_snapshot_t12`.

**Status:** done

### 8.4.9.1 Best-Effort StatefulWriter (T1-T6)

**Spec:** §8.4.9.1.

**Repo:** `crates/rtps/src/writer.rs::Writer` (Best-Effort
StatefulWriter mit per-target Reader-ID + ReaderProxy-Liste in
SPDP/SEDP). T1-T6 Transitions: pushing → DATA, neuer Sample →
new_change, ChangeFromWriter-Status updates, Filter-Drop via
`ChangeKind::AliveFiltered`.

**Tests:** `writer::tests` (Best-Effort-Lifecycle),
`writer_proxy::tests::change_from_writer_*`,
`history_cache::tests::change_kind_alive_filtered_is_alive_but_not_relevant`.

**Status:** done

### 8.4.9.2 Reliable StatefulWriter (T1-T16)

**Spec:** §8.4.9.2.

**Repo:** `crates/rtps/src/reliable_writer.rs::ReliableWriter` mit
allen 16 Transitions: tick → DATA-Push, periodischer HEARTBEAT
(FinalFlag default NOT_SET → ACKNACK erforderlich), handle_acknack
mit GAP-Handling, retransmit-Pfad, AckedByAll-Aussage,
filteredCount-Tracking via `AliveFiltered`-ChangeKind in
ReaderProxy.

**Tests:** `reliable_writer::tests` (40+ Tests fuer alle 16
Transitions inkl. handle_acknack-Variantenpfade, heartbeat-tick,
retransmits, AckedByAll). Plus `tests/reliable_e2e.rs` E2E-Pfad
mit 0%/10%/30% Packet-Loss.

**Status:** done

---

## §8.4.10-12 RTPS Reader / StatelessReader / StatefulReader

### 8.4.10.1 Reader-Attrs (heartbeatResponseDelay/heartbeatSuppressionDuration/expectsInlineQos)

**Spec:** §8.4.10.1 — Default heartbeatResponseDelay=500ms,
heartbeatSuppressionDuration=0.

**Repo:** `reliable_reader.rs`.

**Tests:** —

**Status:** done

### 8.4.10.2 StatelessReader

**Spec:** §8.4.10.2.

**Repo:** SPDP-Reader in `crates/discovery/src/spdp.rs`.

**Tests:** —

**Status:** done

### 8.4.10.3 StatefulReader (matched_writers)

**Spec:** §8.4.10.3.

**Repo:** `reliable_reader.rs`.

**Tests:** —

**Status:** done

### 8.4.10.4 WriterProxy (remoteWriterGuid + Locator-Lists + dataMaxSize + changes_from_writer)

**Spec:** §8.4.10.4.

**Repo:** `writer_proxy.rs`.

**Tests:** —

**Status:** done

### 8.4.10.4.2 available_changes_max

**Spec:** §8.4.10.4.2.

**Repo:** `writer_proxy.rs`.

**Tests:** —

**Status:** done

### 8.4.10.4.3 not_available_change_set + filteredCount (NA_FILTERED/NA_REMOVED/NA_UNSPECIFIED)

**Spec:** §8.4.10.4.3.

**Repo:** `crates/rtps/src/writer_proxy.rs::WriterProxy::
mark_not_available` mit allen 3 Reasons. `filteredCount` wird via
`ChangeKind::AliveFiltered` gefuehrt:
`crates/rtps/src/history_cache.rs::ChangeKind::is_relevant` liefert
`false` fuer AliveFiltered, sodass diese SNs als "not_available" mit
Reason `NA_FILTERED` zaehlen ohne erneute NACKs auszuloesen.
GAP-Submessages mit `filtered_count` (K-Flag) tragen den Counter
zwischen Writer und Reader.

**Tests:** `writer_proxy::tests::not_available_*`,
`history_cache::tests::change_kind_alive_filtered_is_alive_but_not_relevant`,
`submessages::tests::gap_with_filtered_count_roundtrip_le`,
`gap_filtered_count_zero_is_distinct_from_none`.

**Status:** done

### 8.4.10.4.4-7 lost_changes_update / missing_changes / received_change_set

**Spec:** §8.4.10.4.4-7.

**Repo:** `reliable_reader.rs`.

**Tests:** —

**Status:** done

### 8.4.10.5 ChangeFromWriter (status + is_relevant)

**Spec:** §8.4.10.5.

**Repo:** `crates/rtps/src/writer_proxy.rs::ChangeFromWriter` mit
`status` (Missing/Received/Lost/Unknown) + `is_relevant: bool`.
`is_relevant=false` zeigt einen Filter-gedroppten Sample an
(TIME_BASED_FILTER, ContentFilteredTopic, oder
ChangeKind::AliveFiltered). `crates/rtps/src/history_cache.rs::
ChangeKind::is_relevant` ist die kanonische Quelle der
Truth (false nur fuer AliveFiltered).

**Tests:** `writer_proxy::tests` (12+ Tests inkl.
`gap_marks_irrelevant`, `received_change_set` etc.),
`history_cache::tests::change_kind_alive_is_relevant_and_alive`,
`change_kind_alive_filtered_is_alive_but_not_relevant`,
`change_kind_not_alive_kinds_are_not_alive`.

**Status:** done

### 8.4.11 StatelessReader Behavior

**Spec:** §8.4.11.

**Repo:** SPDP-Reader.

**Tests:** —

**Status:** done

### 8.4.12 StatefulReader Behavior

**Spec:** §8.4.12.

**Repo:** `crates/rtps/src/reliable_reader.rs::ReliableReader` mit
matched_writers (Vec<WriterProxy>), pre-emptive ACKNACK,
periodischem ACKNACK-Tick, GAP-Verarbeitung. `Requested`-Sub-State
ist im `WriterProxy.requested_changes` (BTreeSet von SNs ohne
Status-Update) modelliert.

**Tests:** `reliable_reader::tests::pre_emptive_acknack_emitted_after_add_writer_proxy`,
`writer_proxy::tests::missing_changes_respects_max_count`,
`received_removes_from_missing`,
`gap_marks_irrelevant`,
plus 30+ weitere ReliableReader-Tests.

**Status:** done

---

## §8.4.13 Writer Liveliness Protocol

### 8.4.13 WLP via ParticipantMessageData

**Spec:** §8.4.13.

**Repo:** `crates/rtps/src/participant_message_data.rs::
ParticipantMessageData` mit allen 3 Kinds:
`PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE` (kind=0),
`PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE`
(kind=1), `PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC`
(0x80000001 vendor-kind). DCPS-Wiring in
`crates/dcps/src/wlp.rs::WlpEndpoint` mit periodischem AUTOMATIC-
Beat (Tick-getrieben), Manual-Pulse-Queue (`MAX_QUEUED_PULSES=32`),
Per-Peer-Tracking + Lease-Expiry via `lost_peers(now, lease)`.

**Tests:** `participant_message_data::tests` (Wire-Roundtrip aller
3 Kinds), `dcps::wlp::tests`:
`wlp_first_tick_emits_automatic_heartbeat`,
`wlp_assert_participant_emits_manual_pulse`,
`wlp_assert_topic_emits_vendor_kind_with_token`,
`wlp_handle_datagram_updates_peer_state`,
`wlp_lost_peers_returns_only_expired`,
`wlp_pending_queue_caps_at_max` (DoS-Cap).

**Status:** done

---

## §8.4.14 Optional Behavior (Large Data / Fragmentation)

### 8.4.14 Fragmentation via DATA_FRAG/NACK_FRAG/HEARTBEAT_FRAG

**Spec:** §8.4.14.

**Repo:** `fragment_assembler.rs` + Frag-Submessages.

**Tests:** Frag-Tests inkl. DoS-Cap.

**Status:** done

---

## §8.4.15 Implementation Guidelines

### 8.4.15.6 Inactive-Reader-Reclaim (Strict-Reliability-Aussetzer)

**Spec:** §8.4.15.6 — Writer DARF einen Reader-Proxy reclaimen, wenn
dieser laenger als `inactive_threshold` keine ACKNACK / NACK_FRAG
geliefert hat (Strict-Reliability-Aussetzer als DoS-Schutz gegen
stalled Reader).

**Repo:** `crates/rtps/src/reader_proxy.rs::ReaderProxy.last_activity`
+ Methoden `note_activity(now)` (vom ACKNACK/NACK_FRAG-Receive-Pfad
gerufen), `is_inactive(now, threshold)` (Reclaim-Predicate),
`last_activity()` (Diagnose). Caller (`ReliableWriter`) ruft
`is_inactive` periodisch auf und kann den Proxy aus
`matched_readers` entfernen.

**Tests:** `reader_proxy::tests::proxy_is_inactive_initially_when_threshold_is_short`,
`proxy_is_active_after_note_activity`,
`proxy_becomes_inactive_after_threshold_elapses`,
`proxy_inactivity_not_reported_when_now_before_last_activity` (Negativ).

**Status:** done

### 8.4.15.7 Count-Modular-Arithmetik

**Spec:** §8.4.15.7 — Heartbeat/AckNack/NackFrag Counter MUSS
wrap-around-tolerant sein (modulo 2^32, in Rust via
`wrapping_add`).

**Repo:** `crates/rtps/src/reliable_writer.rs::ReliableWriter.
heartbeat_count` + `nackfrag_count` nutzen
`i32::wrapping_add(1)` bei jedem Increment.
`reliable_stateless_writer.rs::ReliableStatelessWriter.
heartbeat_count` nutzt `u32::wrapping_add(1)`. Spec-konform: bei
Erreichen des Maximums wird auf MIN/0 gewrapped.

**Tests:** `reliable_writer::tests::heartbeat_count_increments`,
`heartbeat_count_wraps_around_at_i32_max_per_spec_8_4_15_7`,
`reliable_stateless_writer::tests::heartbeat_count_wraps_at_u32_max_t3_modular`.

**Status:** done

---

## §8.5 Discovery Module

### 8.5.1 SPDP — Simple Participant Discovery Protocol

**Spec:** §8.5.1, S. 117-120.

**Repo:** `crates/discovery/src/spdp.rs`.

**Tests:** SPDP-Tests + Cyclone-Live-SPDP.

**Status:** done

### 8.5.2 SEDP — Simple Endpoint Discovery Protocol

**Spec:** §8.5.2, S. 121-127.

**Repo:** `crates/discovery/src/sedp/`.

**Tests:** SEDP-Tests + Cyclone-Live-SEDP.

**Status:** done

### 8.5.3 ParticipantBuiltinTopicData / PublicationBuiltinTopicData / SubscriptionBuiltinTopicData / TopicBuiltinTopicData

**Spec:** §8.5.3.

**Repo:** Alle 4 Builtin-Topic-Data-Strukturen voll spec-konform:
- `crates/rtps/src/participant_data.rs::ParticipantBuiltinTopicData`
- `crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData`
- `crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData`
  (inkl. `content_filter: Option<ContentFilterProperty>`)
- `crates/dcps/src/builtin_topics.rs::TopicBuiltinTopicData`
  (DCPS-Sicht; SEDP-Topic-Endpoint wird als optionaler Endpoint
  in §8.5.4 announced).

**Tests:** `crates/rtps/src/participant_data.rs::tests::*`,
`publication_data::tests::*`, `subscription_data::tests::*`,
`crates/dcps/src/builtin_topics.rs::tests::*` (Wire-Roundtrip aller 4).

**Status:** done

### 8.5.4 Built-in Endpoints (Reader/Writer pro PDP/SEDP-Topic)

**Spec:** §8.5.4.

**Repo:** Alle Standard-Builtin-Endpoint-Paare als
`EntityId`-Konstanten in `crates/rtps/src/wire_types.rs`:
- SPDP-W/R: `SPDP_BUILTIN_PARTICIPANT_{WRITER,READER}` (0x000100C2/C7).
- SEDP-Pub-W/R: `SEDP_BUILTIN_PUBLICATIONS_{WRITER,READER}` (0x000003C2/C7).
- SEDP-Sub-W/R: `SEDP_BUILTIN_SUBSCRIPTIONS_{WRITER,READER}` (0x000004C2/C7).
- WLP-W/R: `BUILTIN_PARTICIPANT_MESSAGE_{WRITER,READER}` (0x000200C2/C7).
- DDS-Security: SPDP-Secure / SEDP-Pub/Sub-Secure / Stateless / Volatile-
  Secure (12 Endpoints, alle in `wire_types.rs`).

**Tests:** `wire_types::tests::spdp_builtin_*`, `sedp_builtin_*`,
`all_12_secure_entityids_roundtrip`,
`crates/discovery/src/sedp/stack::tests::*`,
`spdp::tests::*`.

**Status:** done

### 8.5.5 Lease-Renewal + Cleanup bei Expiry

**Spec:** §8.5.5.

**Repo:** `crates/discovery/src/spdp.rs::DiscoveredParticipantsCache`
mit `last_seen`-Tracking + Lease-Expiry-Iteration.
`crates/dcps/src/wlp.rs::WlpEndpoint::lost_peers(now, lease)` liefert
expired Peers an den Caller.

**Tests:** `wlp::tests::wlp_lost_peers_returns_only_expired`,
`wlp_handle_datagram_updates_peer_state`,
`wlp_forget_peer_removes_state`,
`spdp::tests::*` (cache lifecycle).

**Status:** done

---

## §8.6 Versioning and Extensibility

### 8.6 Vendor-Extensions + Submessage-Erweiterungen

**Spec:** §8.6 — Vendor-spezifische Submessage-IDs (0x80-0xFF) sowie
Vendor-spezifische PIDs (PID-Bit 15 gesetzt) MUESSEN von Receivers
ohne Must-Understand-Bit ignoriert werden; mit Must-Understand-Bit
darf der Receiver die Message verwerfen.

**Repo:** `crates/rtps/src/submessage_header.rs::SubmessageId`-Enum
mit Standard-Range + Pass-through fuer Vendor-Range via
`SUBMESSAGE_ID_HEADER_EXTENSION` (0x80) und Unknown-Variante.
Vendor-spezifischer PID-Pfad in
`crates/rtps/src/parameter_list.rs::VENDOR_SPECIFIC_BIT` (0x8000) +
`validate_must_understand_in_data_pipeline` (silent-skip fuer
vendor-spezifische MU-PIDs, Spec §9.6.2).

**Tests:** `datagram::tests::decode_marks_unknown_submessage_id_without_failing`,
`decode_skips_unknown_submessage_without_must_understand`,
`decode_rejects_unknown_submessage_with_must_understand` (Negativ),
`parameter_list::tests::must_understand_vendor_pid_with_must_understand_rejects`,
`validate_must_understand_in_data_pipeline_vendor_specific_pid_passes`,
`datagram::tests::decode_accepts_vendor_specific_must_understand_pid`.

**Status:** done

---

## §8.7 DDS QoS via RTPS

### 8.7.1 PID-Tabelle (alle DDS-QoS-PIDs)

**Spec:** §8.7.1, S. 131-138.

**Repo:** `crates/rtps/src/parameter_list.rs::pid` mit allen Standard-
PIDs inkl. neu in K3b-F: `TYPE_MAX_SIZE_SERIALIZED` (0x0060),
`ORIGINAL_WRITER_INFO` (0x0061), `WRITER_GROUP_INFO` (0x0062). Plus
`COHERENT_SET` / `GROUP_COHERENT_SET` / `GROUP_SEQ_NUM` /
`DIRECTED_WRITE` / `KEY_HASH` / `STATUS_INFO` etc. (50+ PIDs total).
`is_standard_pid(pid)` als Klassifikator fuer Must-Understand-Reject.

**Tests:** `parameter_list::tests::is_standard_pid_recognises_dds_security_pids`,
`is_standard_pid_unknown_pid_returns_false` (Negativ),
plus alle ParameterList-Roundtrip-Tests pro PID-Familie.

**Status:** done

### 8.7.2 LIVELINESS Wire-Mapping (ParticipantMessageData + Heartbeat-LivelinessFlag)

**Spec:** §8.7.2.

**Repo:** AUTOMATIC + MANUAL_BY_PARTICIPANT als
`ParticipantMessageData`-DATA-Sample im
`BUILTIN_PARTICIPANT_MESSAGE_WRITER`-Stream
(`crates/rtps/src/participant_message_data.rs::ParticipantMessageData`).
MANUAL_BY_TOPIC via Heartbeat-LivelinessFlag (L-Bit, Spec §8.3.8.6) +
ZeroDDS-Vendor-Kind (`PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC`)
mit Topic-Token. Reliable-Writer setzt L-Flag auf Heartbeats fuer
MANUAL_BY_TOPIC-konfigurierte Writers.

**Tests:** `participant_message_data::tests::*`,
`submessages::tests::heartbeat_submessage_liveliness_flag_roundtrip`,
`dcps::wlp::tests::wlp_assert_topic_emits_vendor_kind_with_token`,
`wlp_assert_participant_emits_manual_pulse`.

**Status:** done

### 8.7.3 Content-filtered Topics (Reader-side Filter + ContentFilterProperty_t)

**Spec:** §8.7.3.

**Repo:** `crates/rtps/src/subscription_data.rs::ContentFilterProperty`
+ Wire-Encode/Decode (`encode_content_filter_property_le` /
`decode_content_filter_property`). PID `CONTENT_FILTER_PROPERTY`
(0x0035) in SubscriptionBuiltinTopicData. Reader-side Filter via
`crates/dcps/src/topic.rs::ContentFilteredTopic` mit SQL-Filter
ueber `zerodds-sql-filter`-Crate (siehe DCPS K3a §2.2.2.5.4 +
ContentFilteredTopic-Tests).

**Tests:** `crates/rtps/src/subscription_data.rs::tests::*`
(ContentFilterProperty Wire-Roundtrip),
`crates/dcps/src/topic.rs::tests::cft_*` (Filter-Logik).

**Status:** done

### 8.7.4 Instance-State-Transitions

**Spec:** §8.7.4.

**Repo:** `crates/dcps/src/instance_tracker.rs::InstanceTracker` +
`InstanceState`. Spec-Transitions ALIVE → NOT_ALIVE_DISPOSED /
NOT_ALIVE_NO_WRITERS / ALIVE_FILTERED ueber `dispose()`,
`unregister()`, `should_deliver_under_time_based_filter`. Tracking
ueber `disposed_at`, `no_writers_at`, `current_owner` mit
generation-Counters. `ChangeKind::AliveFiltered` zaehlt im
filteredCount aber bleibt instance-aktiv.

**Tests:** `instance_tracker::tests::dispose_transitions_to_disposed`,
`re_register_after_dispose_bumps_disposed_generation`,
`unregister_decrements_writer_count`,
`time_based_filter_*` (5 Tests),
`autopurge_*` (5 Tests),
`exclusive_*` (8 Tests Strength-Selection).

**Status:** done

### 8.7.5 Group-Ordered Access (PID_GROUP_SEQ_NUM/PID_WRITER_GROUP_INFO/Heartbeat.GSN)

**Spec:** §8.7.5.

**Repo:** PIDs in `crates/rtps/src/parameter_list.rs::pid::
{GROUP_SEQ_NUM, WRITER_GROUP_INFO}`. Heartbeat-GroupInfo-Sektion
(GSN + group_first/last) in
`crates/rtps/src/submessages.rs::HeartbeatSubmessage::group_info`
(G-Flag). DCPS-side GROUP-coherent-Pfad in
`crates/dcps/src/coherent_set.rs::GroupAccessScope.snapshot_generation`
(siehe DCPS K3a §2.2.3.6).

**Tests:** `submessages::tests::heartbeat_with_writer_set_roundtrip_be`,
`heartbeat_with_empty_group_info_roundtrip_le`,
`coherent_set::tests::snapshot_generation_*` (DCPS),
`is_standard_pid_recognises_*` (PID-Acceptance).

**Status:** done

### 8.7.6 Coherent Sets (PID_COHERENT_SET/PID_GROUP_COHERENT_SET/ECS-Sample)

**Spec:** §8.7.6.

**Repo:** Inline-QoS-PIDs `COHERENT_SET` (0x0056) + `GROUP_COHERENT_SET`
(0x0063) in `crates/rtps/src/parameter_list.rs::pid`.
DCPS-side Coherent-Scope-Wiring in
`crates/dcps/src/coherent_set.rs::{CoherentScope, CoherentSetMarker,
GroupAccessScope}` mit `snapshot_generation` (atomic Cut beim
GROUP-coherent begin_access). End-of-Coherent-Set (ECS) Sample wird
durch end_access mit Generation-Bump signalisiert.

**Tests:** `coherent_set::tests::*` (snapshot-generation +
multi-reader-shared-scope + nested begin/end).

**Status:** done

### 8.7.7 Directed Write

**Spec:** §8.7.7 / §9.6.2.2.5 — Inline-QoS `PID_DIRECTED_WRITE`
adressiert ein Sample auf einen einzigen Reader-GUID; andere
Reader, die das Sample (z.B. via Multicast) empfangen, MUESSEN
verwerfen.

**Repo:** `crates/rtps/src/parameter_list.rs::pid::DIRECTED_WRITE`
(0x0057). Encoder/Filter in
`crates/rtps/src/inline_qos.rs::directed_write_param`,
`find_directed_write`, `directed_write_matches_reader`.

**Tests:** `inline_qos::tests::directed_write_param_carries_target_guid`,
`find_directed_write_returns_target_when_present`,
`find_directed_write_returns_none_when_absent` (Negativ),
`find_directed_write_rejects_truncated_value` (Negativ),
`directed_write_matches_reader_returns_true_for_matching_target`,
`directed_write_matches_reader_returns_false_for_other_target` (Negativ),
`directed_write_matches_reader_returns_true_when_no_directed_write`.

**Status:** done

### 8.7.8 Inline-QoS-Pfad

**Spec:** §8.7.8.

**Repo:** `crates/rtps/src/inline_qos.rs` mit Helper-Funktionen fuer
alle Standard-Inline-QoS-PIDs:
`reply_inline_qos` (PID_RELATED_SAMPLE_IDENTITY) + `find_*` Decoder,
`directed_write_param` (PID_DIRECTED_WRITE) +
`find_directed_write` + `directed_write_matches_reader`,
`original_writer_info_param` (PID_ORIGINAL_WRITER_INFO) +
`find_original_writer_info`. PID_KEY_HASH Inline-QoS in
`crates/dcps/src/dds_type.rs::DdsType::compute_key_hash`. PID_STATUS_INFO
+ PID_PROPERTY_LIST in `crates/rtps/src/property_list.rs` +
`crates/dcps/src/dds_type.rs`.

**Tests:** `inline_qos::tests::*` (28 Tests inkl. Directed-Write,
OriginalWriterInfo, RelatedSampleIdentity Roundtrip).

**Status:** done

### 8.7.9 OriginalWriterInfo (fuer Persistence-Service-Forwarding)

**Spec:** §8.7.9 — Inline-QoS `PID_ORIGINAL_WRITER_INFO` (24 byte:
GUID + i64 SequenceNumber). Persistence-Service setzt das auf
weitergeleiteten Late-Joiner-Replay-Samples.

**Repo:** `crates/rtps/src/parameter_list.rs::pid::ORIGINAL_WRITER_INFO`
(0x0061), `crates/rtps/src/inline_qos.rs::original_writer_info_param`
(Encoder), `find_original_writer_info` (Decoder mit LE/BE-Support).
Persistence-Service-Backend in `crates/dcps/src/durability_service.rs`
(siehe DCPS K3a §2.2.3.5).

**Tests:** `inline_qos::tests::original_writer_info_param_24_byte_layout_le`,
`original_writer_info_roundtrip_le`,
`original_writer_info_roundtrip_be`,
`find_original_writer_info_returns_none_when_absent` (Negativ),
`find_original_writer_info_rejects_truncated` (Negativ).

**Status:** done

---

## §9 PSM UDP/IP

### 9.1-9.3 Notational Conventions / RTPS Types Mapping

**Spec:** §9.1-9.3, S. 143-152.

**Repo:** `wire_types.rs`.

**Tests:** —

**Status:** done

### 9.3.1 GUID-PSM-Mapping (auto-generated von DDS-Implementation)

**Spec:** §9.3.1.

**Repo:** `wire_types.rs::Guid`.

**Tests:** —

**Status:** done

### 9.3.2 IDL-Definitionen / SubmessageElement-Typen

**Spec:** §9.3.2 — Built-in Endpoint-Set Bits 0-29.

**Repo:** `crates/rtps/src/participant_data.rs::BuiltinEndpointSet`-
Bitfeld mit allen Bits: SPDP-W/R (0/1), SEDP-Pub-W/R (2/3), SEDP-
Sub-W/R (4/5), SEDP-Topic-W/R (6/7), Participant-Message W/R (10/11),
DDS-Security-Endpoints (16-23), Topic-Discovery (28/29).

**Tests:** `participant_data::tests::*` (BuiltinEndpointSet
Wire-Roundtrip pro Bit).

**Status:** done

### 9.4 Mapping der Messages (Wire-Encoding aller Submessages)

**Spec:** §9.4 — Wire-Layout pro Submessage.

**Repo:** `crates/rtps/src/submessages.rs` + `header_extension.rs` +
`datagram.rs` mit kompletten Encode/Decode-Pfaden fuer alle
Submessage-Klassen (DATA, DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG,
ACKNACK, NACK_FRAG, GAP, INFO_TS, INFO_SRC, INFO_DST, INFO_REPLY,
PAD, HEADER_EXTENSION). InfoReplyIp4 wird als generisches
`InfoReplySubmessage` mit IPv4-Locator-Variante codiert.

**Tests:** `submessages::tests::*` (60+ Wire-Roundtrip-Tests),
`header_extension::tests::*` (16 HE-Roundtrip-Tests),
`datagram::tests::*` (Datagram-Composition).

**Status:** done

### 9.4.2.11.2 Must-Understand-Bit (PID & 0x4000)

**Spec:** §9.4.2.11.2 — "If the receiver does not understand a
parameter and the must_understand bit (0x4000) is set, the entire
RTPS message carrying this ParameterList MUST be discarded."

**Repo:** `crates/rtps/src/parameter_list.rs::ParameterList::
validate_must_understand_in_data_pipeline` mit
`is_standard_pid(masked_pid)`-Klassifikator (alle 50+ Spec-PIDs
inkl. DDS-Security 1.2). Wire-Hot-Path-Aktivierung in
`crates/rtps/src/datagram.rs::decode_datagram` — sowohl auf der
DATA-Submessage's `inline_qos` als auch auf der HE.parameters.
Vendor-spezifische PIDs (oberstes Bit gesetzt) werden auch mit
Must-Understand-Bit ignoriert (Spec §9.6.2).

**Tests:** `parameter_list::tests::must_understand_known_pid_passes`,
`must_understand_unknown_pid_rejects` (Negativ),
`must_understand_vendor_pid_with_must_understand_rejects` (Negativ),
`validate_must_understand_in_data_pipeline_known_pid_passes`,
`validate_must_understand_in_data_pipeline_unknown_pid_rejects` (Negativ),
`validate_must_understand_in_data_pipeline_vendor_specific_pid_passes`,
`validate_must_understand_in_data_pipeline_optional_unknown_pid_passes`,
`is_standard_pid_recognises_dds_security_pids`,
`is_standard_pid_unknown_pid_returns_false` (Negativ),
`datagram::tests::decode_rejects_data_with_unknown_must_understand_pid_in_inline_qos` (Negativ),
`decode_accepts_data_with_known_must_understand_pid_in_inline_qos`,
`decode_accepts_vendor_specific_must_understand_pid`.

**Status:** done

### 9.4.2.15 Checksum-Pfad (CRC-32C / CRC-64-XZ / MD5-128)

**Spec:** §9.4.2.15 — alle 3 Checksum-Algorithmen via
HeaderExtension.C-Flag-Paerchen (C0+C1).

**Repo:** `crates/rtps/src/header_extension.rs::ChecksumKind` mit allen
4 Werten (None/Crc32c/Crc64/Md5) + `ChecksumValue` Enum-Variants
mit dem Hash-Wert (4/8/16 byte). Wire-Encode/Decode in `decode_body`
+ `flag_byte`.

**Compute/Verify:** `ChecksumValue::compute(kind, payload)` und
`ChecksumValue::verify(payload)` berechnen die Checksum bzw. pruefen
einen empfangenen Wert gegen einen Payload-Slice. Die drei Algorithmen
sind durchgehend pure-Rust no_std implementiert via
`zerodds_foundation::{crc32c, crc64_xz, md5}` — keine externe Crypto-Crate-
Abhaengigkeit (Pillar 9 Zero-Dependency).

**Tests:** `header_extension::tests::roundtrip_with_crc32c_le`,
`roundtrip_with_crc64_be`,
`roundtrip_with_md5_le`,
`checksum_kind_roundtrip` (alle 4 Kinds),
`checksum_kind_wire_size`,
`decode_rejects_truncated_md5_checksum_body` (Negativ),
`checksum_compute_crc32c_matches_rfc4960_vector` (RFC 4960 App. B Test-Vector),
`checksum_compute_crc64_xz_matches_known_vector` (XZ utils Standard-Vector),
`checksum_compute_md5_matches_rfc1321_vector` (RFC 1321 §A.5 Test-Vector),
`checksum_verify_round_trip_crc32c_succeeds`,
`checksum_verify_detects_tampered_payload` (Negativ),
`checksum_verify_none_kind_always_succeeds`,
`checksum_verify_crc64_round_trip`,
`checksum_verify_md5_round_trip`,
`checksum_compute_none_yields_none`.

**Status:** done

### 9.5 Mapping zu UDP/IP Transport Messages

**Spec:** §9.5.

**Repo:** `crates/transport-udp/`.

**Tests:** Transport-Tests.

**Status:** done

### 9.6 RTPS Protocol Mapping (Port-Berechnung PB=7400/DG=250/PG=2/d0..d3 + Multicast-Locator)

**Spec:** §9.6.

**Repo:** `crates/discovery/src/spdp.rs::well_known_port`.

**Tests:** Port-Tests.

**Status:** done

### 9.6.3 Optimization-Omit (MUSS-omit redundante Felder bei SPDP/SEDP)

**Spec:** §9.6.3 — Felder die beim Default-Wert nicht in PL-CDR
serialisiert werden duerfen (Bandbreiten-Optimierung).

**Repo:** `crates/rtps/src/participant_data.rs::ParticipantBuiltinTopicData::
to_pl_cdr_le` + `crates/rtps/src/publication_data.rs` /
`subscription_data.rs` lassen Default-QoS-Werte weg
(`if let Some(...)` Pattern), schreiben nur explizit gesetzte
QoS-Policies. PIDs `IGNORE` (0x3F03) als Padding-Marker.

**Tests:** `participant_data::tests::*` (Wire-Roundtrip ohne
Default-Werte byte-identisch zu Cyclone-Sample).

**Status:** done

### 9.6.4.3 KeyHash-Berechnung (PLAIN_CDR2-BE-Serialization)

**Spec:** §9.6.4.3 + XTypes 1.3 §7.6.8 — KeyHash = PLAIN_CDR2-BE-
Serialization der @key-Member, sortiert nach memberId. Bei
max_size > 16: MD5(stream)[0..16]. Sonst zero-padded.

**Repo:** `crates/cdr/src/key_hash.rs::compute_key_hash` mit beiden
Pfaden (zero-pad + MD5). XCDR2-BE-Serialization mit Member-Reorder
durch `dds_type::DdsType::encode_key_holder_be` (proc-macro generiert
sortierte Reihenfolge in K2 XTypes-Voll-Stack).

**Tests:** `cdr::key_hash::tests::*` (16+ Tests fuer beide Pfade,
zero-pad + MD5),
`dds_type::tests::small_keyed_produces_zero_padded_keyhash`,
`large_keyed_produces_md5_hashed_keyhash`,
`keyed_member_order_matters` (Sort-Stability).

**Status:** done

---

## §10 SerializedPayload Representation

### 10.1-10.7 SerializedPayloadHeader + Repr-Identifier (CDR/PL_CDR/CDR2/D_CDR/XML)

**Spec:** §10, S. 193-199.

**Repo:** `crates/cdr/src/representation.rs::RepresentationIdentifier`
mit allen Spec-Werten (CDR_BE/LE, PL_CDR_BE/LE, CDR2_BE/LE,
PL_CDR2_BE/LE, D_CDR_BE/LE, XML, RTPS_HE 0xC0xx). 4-byte
Encapsulation-Header (RID + Options) als Prefix jeder
SerializedPayload. D-CDR (Delimited) als Length-Prefix-Wrapper
ueber XCDR2 (XTypes 1.3 §7.4.3.5.4) im DCPS-CDR-Layer
implementiert.

**Tests:** `cdr::representation::tests::*` (Wire-Roundtrip aller
Identifier inkl. Options-Bits), `cdr::xcdr2::tests::*` (XCDR2-BE
+ LE), `cdr::tests::pl_cdr::*` (PL_CDR-Pfad).

**Status:** done

---

## Audit-Status

121 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test-Lauf:

* `cargo test -p zerodds-rtps` — 572 lib + 54 integration
  (8 Bins: compliance_rtps, cyclone_compliance,
  cyclone_he_must_understand, fixture_loader, fuzz_smoke,
  proptest_roundtrip, reliable_e2e, sedp_qos_roundtrip)
  = 626 Tests grün.
* `cargo test -p zerodds-discovery` — 111 lib + 43 integration
  (10 Bins: cyclone_full_interop, cyclone_live_sedp,
  cyclone_live_typelookup, cyclone_sedp_replay,
  cyclone_typelookup_responder, fastdds_live_spdp, fuzz_smoke,
  sedp_loopback, spdp_loopback_e2e, type_lookup_service)
  = 154 Tests grün (SPDP + SEDP + ParameterList +
  TypeLookup-Service).
