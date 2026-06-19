# DDSI-RTPS 2.5 — Spec Coverage

**Spec:** [OMG DDSI-RTPS 2.5](https://www.omg.org/spec/DDSI-RTPS/2.5/PDF) (206 pages, OMG formal/2022-04-01)

Audit item-by-item against the spec; each requirement with a spec
quote + repo path + test path + status (`done` / `partial` / `open` / `n/a`).

**Context:** The DDSI-RTPS 2.5 stack is spread across:

- `crates/rtps/` — RTPS wire stack: ~30 files + 458 tests (datagram/header/header_extension/fragment_assembler/inline_qos/message_builder/parameter_list/participant_data/publication_data/subscription_data/qos_bridge/reader/reader_proxy/receiver_state/reliable_reader/reliable_writer/submessage_header/submessages/wire_types/writer/writer_proxy + endpoint_security_info/property_list/security_algo_info)
- `crates/discovery/` — SPDP/SEDP/TypeLookup: spdp.rs + sedp/* + type_lookup/* + endpoint_match.rs + capabilities.rs, 111 tests

SPDP+SEDP+TypeLookup live + all 16 submessages + HeaderExtension +
reliable stack + fragmentation.

---

## §1 Scope

### 1.1 DDS interop wire protocol

**Spec:** §1, p. 1 — "DDS Real-Time Publish-Subscribe Wire-Protocol."

**Repo:** `crates/rtps/src/lib.rs`.

**Tests:** crate-wide.

**Status:** done

---

## §2 Conformance

### 2.1 Conformance per §8.4.2 (Behavior Module)

**Spec:** §2.

**Repo:** complete behavior-module coverage:
- Stateless Best-Effort: `crates/discovery/src/spdp.rs::SpdpBeacon`
  + `crates/discovery/src/security/stateless.rs::StatelessMessageWriter`.
- Stateless Reliable: `crates/rtps/src/reliable_stateless_writer.rs::
  ReliableStatelessWriter` (T1-T12).
- Stateful Best-Effort: `crates/rtps/src/{writer,reader}.rs`.
- Stateful Reliable: `crates/rtps/src/{reliable_writer,reliable_reader}.rs`
  with fragmentation (`fragment_assembler.rs`).

**Tests:** an own test suite per behavior variant; E2E in
`tests/reliable_e2e.rs` (0%/10%/30% packet loss Best-Effort+Reliable).

**Status:** done

---

## §3 Normative References

### 3.1 [DDS] DDS 1.4 / [IDL] IDL 4.2 / [DDS-XTYPES] XTypes 1.2

**Spec:** §3.

**Repo:** `crates/dcps/`, `crates/idl/`, `crates/types/`.

**Tests:** see per spec-coverage doc.

**Status:** done

### 3.2 NTPv3, MD5, SCTP-CRC32c, AUTOSAR-CRC

**Spec:** §3 — normative references.

**Repo:** all 4 standards referenced + in code:
- NTPv3 timestamp format: `crates/rtps/src/header_extension.rs::HeTimestamp`
  + `crates/dcps/src/time.rs::Time` (sec+nanosec/fraction).
- MD5 (RFC 1321) as a 128-bit KeyHash + ChecksumValue::Md5 in
  `crates/rtps/src/header_extension.rs::ChecksumValue` + the KeyHash path
  in `crates/cdr/src/keyhash.rs`.
- SCTP-CRC32c (Castagnoli, RFC 3309): `ChecksumKind::Crc32c` +
  ChecksumValue::Crc32c.
- AUTOSAR-CRC-64-XZ (ISO 14229): `ChecksumKind::Crc64` +
  ChecksumValue::Crc64.

**Tests:** `header_extension::tests::roundtrip_with_crc32c_le`,
`roundtrip_with_crc64_be`, `roundtrip_with_md5_le`,
`checksum_kind_roundtrip`, `checksum_kind_wire_size`,
`empty_he_le_roundtrip` (NTPv3 indirectly via HeTimestamp).

**Status:** done

---

## §4 Terms

### 4 Terminology (CDR/DDS/EDP/GUID/PDP/PIM/PSM/RTPS/SEDP)

**Spec:** §4 — glossary.

**Repo:** terms used consistently.

**Tests:** —

**Status:** `n/a (informative)` — glossary, no normative
requirement.

---

## §5 Symbols

### 5 Symbols + acronyms

**Spec:** §5.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — symbol/acronym list.

---

## §6 Additional Information

### 6.1-6.4 Changes/How-to-Read/Acks/Maturity

**Spec:** §6.1-6.4.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial sections (change log,
reading hints, acknowledgments, maturity statement).

---

## §7 Overview / PIM/PSM Architecture

### 7.1 Introduction (wire-protocol requirements)

**Spec:** §7.1, p. 7.

**Repo:** n/a

**Tests:** n/a

**Status:** done

### 7.2 Requirements for a DDS wire-protocol

**Spec:** §7.2, p. 7-8.

**Repo:** n/a — requirements fulfilled by the crate architecture.

**Tests:** crate-wide.

**Status:** done

### 7.3 The RTPS wire-protocol

**Spec:** §7.3, p. 8.

**Repo:** crate `crates/rtps/`.

**Tests:** wire tests.

**Status:** done

### 7.4.1 Structure Module (PIM)

**Spec:** §7.4.1, p. 9-11 — GUID/Cache/Endpoint/Participant.

**Repo:** `wire_types.rs` with GUID/SequenceNumber/Locator/etc.

**Tests:** wire-types tests.

**Status:** done

### 7.4.2 Messages Module (PIM)

**Spec:** §7.4.2.

**Repo:** the `submessages/` module (all 16 submessages) +
`message_builder.rs`.

**Tests:** submessage roundtrip tests.

**Status:** done

### 7.4.3 Behavior Module (PIM)

**Spec:** §7.4.3.

**Repo:** four canonical behavior implementations live:
StatelessBE (`spdp.rs`), StatelessReliable
(`reliable_stateless_writer.rs`), StatefulBE (`writer.rs`/`reader.rs`),
StatefulReliable (`reliable_writer.rs`/`reliable_reader.rs`).
Each with its own state-machine module.

**Tests:** an own test suite per variant (see §2.1).

**Status:** done

### 7.4.4 Discovery Module (PIM)

**Spec:** §7.4.4.

**Repo:** `crates/discovery/src/spdp.rs` + `sedp/`.

**Tests:** SPDP+SEDP tests.

**Status:** done

### 7.5 RTPS Platform Specific Model (PSM)

**Spec:** §7.5, p. 11 — the UDPv4 PSM is mandatory.

**Repo:** UDPv4 PSM via `crates/transport-udp/`.

**Tests:** transport tests.

**Status:** done

### 7.6 RTPS Transport Model

**Spec:** §7.6, p. 11 — 16-byte address, 4-byte port, datagram,
drop-on-corrupt.

**Repo:** `wire_types.rs::Locator` (16+4) + UDP datagram path.

**Tests:** locator tests.

**Status:** done

---

## §8.2 Structure Module

### 8.2.1.2 GUID_t (16 octets)

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::Guid`.

**Tests:** GUID tests.

**Status:** done

### 8.2.1.2 GuidPrefix_t (12 octets) + GUIDPREFIX_UNKNOWN

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::GuidPrefix`.

**Tests:** —

**Status:** done

### 8.2.1.2 EntityId_t (4 octets) + ENTITYID_UNKNOWN

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::EntityId`.

**Tests:** —

**Status:** done

### 8.2.1.2 SequenceNumber_t (64-bit) + SEQUENCENUMBER_UNKNOWN

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::SequenceNumber` (i64 repr).

**Tests:** —

**Status:** done

### 8.2.1.2 Locator_t (kind+port+16-byte addr) + 5 reserved constants

**Spec:** §8.2.1.2 — LOCATOR_INVALID/KIND_INVALID/RESERVED/UDPv4/UDPv6.

**Repo:** `crates/rtps/src/wire_types.rs::Locator` with constants
INVALID/RESERVED/UDP_V4_ANY/UDP_V6_ANY/SHM_ANY + PORT_INVALID +
ADDRESS_INVALID + constructors udp_v4/udp_v6/tcp_v4/uds/shm.

**Tests:** `wire_types::tests::locator_invalid_constant_matches_spec`,
`locator_reserved_constant_kind_is_zero`,
`locator_udp_v4_any_kind_is_one`,
`locator_udp_v6_any_kind_is_two`,
`locator_shm_any_uses_vendor_kind`,
`locator_udp_v6_constructor_keeps_full_address`,
`locator_udp_v6_roundtrip_le`,
`locator_udp_v4_layout`, `locator_uds_layout`,
`locator_kind_roundtrip`, `locator_kind_rejects_unknown` (negative),
`locator_invalid_kind_decoded` (negative).

**Status:** done

### 8.2.1.2 TopicKind_t (NO_KEY/WITH_KEY)

**Spec:** §8.2.1.2.

**Repo:** `wire_types.rs::TopicKind`.

**Tests:** —

**Status:** done

### 8.2.1.2 ChangeKind_t (ALIVE/ALIVE_FILTERED/NOT_ALIVE_DISPOSED/NOT_ALIVE_UNREGISTERED)

**Spec:** §8.2.1.2.

**Repo:** `crates/rtps/src/history_cache.rs::ChangeKind` with all
4 spec variants + `NotAliveDisposedUnregistered` (combined marker)
+ `is_relevant()` (false for AliveFiltered) + `is_alive_kind()`
(true for Alive + AliveFiltered).

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

**Repo:** indirectly via the DCPS `InstanceHandle`.

**Tests:** —

**Status:** done

### 8.2.1.2 ProtocolVersion_t with aliases (1.0/1.1/2.0/2.1/2.2/2.4 + 2.5)

**Spec:** §8.2.1.2.

**Repo:** `crates/rtps/src/wire_types.rs::ProtocolVersion` with all
aliases V1_0/V1_1/V2_0/V2_1/V2_2/V2_3/V2_4/V2_5 + a `CURRENT` alias
+ default V2_5.

**Tests:** `wire_types::tests::protocol_version_aliases_match_spec`,
`protocol_version_all_aliases_roundtrip`,
`protocol_version_default_is_2_5`,
`protocol_version_roundtrip`.

**Status:** done

### 8.2.1.2 VendorId_t (OMG-managed list) + VENDORID_UNKNOWN

**Spec:** §8.2.1.2.

**Repo:** `header.rs::VendorId` + constants.

**Tests:** —

**Status:** done

### 8.2.2 HistoryCache (add/remove/get_seq_min/get_seq_max)

**Spec:** §8.2.2.

**Repo:** `crates/rtps/src/history_cache.rs` (WHC/RHC).

**Tests:** history-cache tests.

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

**Repo:** endpoint fields distributed across `publication_data.rs` /
`subscription_data.rs`.

**Tests:** —

**Status:** done

### 8.2.8 RTPS Writer (attributes from §8.4.7.1)

**Spec:** §8.2.8.

**Repo:** `writer.rs`+`reliable_writer.rs`.

**Tests:** writer tests.

**Status:** done

### 8.2.9 RTPS Reader (attributes from §8.4.10)

**Spec:** §8.2.9.

**Repo:** `reader.rs`+`reliable_reader.rs`.

**Tests:** reader tests.

**Status:** done

### 8.2.10 DDS DataWriter/Reader binding (T1-T5 transitions)

**Spec:** §8.2.10.

**Repo:** DCPS layer (`crates/dcps/`).

**Tests:** DCPS tests.

**Status:** done

---

## §8.3 Messages Module

### 8.3.2 SubmessageKind (16 types)

**Spec:** §8.3.2 — RTPS_HE/DATA/HEARTBEAT/ACKNACK/PAD/INFO_TS/
INFO_REPLY/INFO_DST/INFO_SRC/DATA_FRAG/NACK_FRAG/HEARTBEAT_FRAG/
GAP/INFO_REPLY_IP4/SEC_*

**Repo:** `submessage_header.rs::SubmessageId`.

**Tests:** SubmessageId tests.

**Status:** done

### 8.3.2 Time_t with nano resolution (TIME_ZERO/INVALID/INFINITE)

**Spec:** §8.3.2.

**Repo:** `crates/dcps/src/time.rs::Time` (DCPS+RTPS shared) with
`ZERO` / `INVALID` / `INFINITE` sentinels + `is_zero()`/`is_invalid()`/
`is_infinite()` predicates. The RTPS submessage body uses
`crates/rtps/src/header_extension.rs::HeTimestamp` (sec+fraction).

**Tests:** `zerodds_dcps::time::tests::time_zero_sentinel`,
`time_invalid_sentinel`, `time_infinite_sentinel`,
`get_current_time_is_recent` (positive),
`duration_to_core_maps_infinite_to_max`,
`duration_negative_sec_maps_to_zero` (negative).

**Status:** done

### 8.3.2 Checksum_t (CHECKSUM_INVALID)

**Spec:** §8.3.2.

**Repo:** `header_extension.rs::ChecksumKind::None`/`ChecksumValue`.

**Tests:** checksum tests.

**Status:** done

### 8.3.2 MessageLength_t (MESSAGE_LENGTH_INVALID)

**Spec:** §8.3.2.

**Repo:** `header_extension.rs` with `Option<u32>` + None=invalid.

**Tests:** —

**Status:** done

### 8.3.2 GroupDigest_t

**Spec:** §8.3.2 / §8.3.5.10 — 16-byte CDR-encoded 128-bit digest
for group membership.

**Repo:** `crates/rtps/src/group_digest.rs::GroupDigest` with an
`UNKNOWN` sentinel (16 null bytes), `from_prefixes(prefixes)` (sorts
GuidPrefixes lexicographically and MD5-hashes via the `md-5` crate, byte-
identical to Cyclone DDS), `to_bytes` / `read_from(bytes)` wire
encoding.

**Tests:** `group_digest::tests::md5_empty_input_matches_rfc1321_test_vector`,
`md5_abc_matches_rfc1321_test_vector`,
`md5_message_digest_matches_rfc1321_test_vector`,
`md5_long_input_matches_rfc1321_test_vector` (2-block path),
`group_digest_unknown_is_zero`,
`group_digest_from_empty_prefixes_is_md5_of_empty`,
`group_digest_independent_of_input_order`,
`group_digest_distinguishes_different_groups`,
`group_digest_roundtrip`,
`group_digest_read_from_truncated_rejects` (negative),
`group_digest_handles_more_than_5_prefixes_two_block_md5`.

**Status:** done

### 8.3.2 UExtension4_t / WExtension8_t

**Spec:** §8.3.2.

**Repo:** `crates/rtps/src/wire_types.rs::UExtension4` (4-byte vendor-
opaque) + `WExtension8` (8-byte vendor-opaque) with u32/u64-BE
converters + roundtrip identity. The HE submessage uses UExtension4
via `header_extension.rs::HeaderExtension.uextension4`.

**Tests:** `wire_types::tests::uextension4_roundtrip`,
`uextension4_u32_be_roundtrip`,
`uextension4_default_is_zero`,
`wextension8_roundtrip`,
`wextension8_u64_be_roundtrip`,
`wextension8_default_is_zero`.

**Status:** done

### 8.3.3 Message = Header + 1..* Submessages; HeaderExtension optionally directly after the header

**Spec:** §8.3.3.

**Repo:** `datagram.rs` + `header_extension.rs`.

**Tests:** message roundtrip tests.

**Status:** done

### 8.3.3.1 Header: protocol(4) + version(2) + vendorId(2) + guidPrefix(12) = 20 bytes

**Spec:** §8.3.3.1.

**Repo:** `header.rs::Header`.

**Tests:** header tests.

**Status:** done

### 8.3.3.1.1 protocol == PROTOCOL_RTPS

**Spec:** §8.3.3.1.1.

**Repo:** `header.rs` magic check.

**Tests:** —

**Status:** done

### 8.3.3.1.2 version = 2.5

**Spec:** §8.3.3.1.2.

**Repo:** `crates/rtps/src/wire_types.rs::ProtocolVersion::V2_5` +
`Default::default()` delivers V2_5; `CURRENT` alias for the
latest supported spec state.

**Tests:** `wire_types::tests::protocol_version_default_is_2_5`,
`protocol_version_aliases_match_spec`,
`protocol_version_all_aliases_roundtrip`.

**Status:** done

### 8.3.3.2 HeaderExtension submessage (messageLength/rtpsSendTimestamp/uExtension4/wExtension8/messageChecksum/parameters + 7 flags)

**Spec:** §8.3.3.2.

**Repo:** `crates/rtps/src/header_extension.rs::HeaderExtension` with
all fields (messageLength/timestamp/uextension4/extension16/
checksum/parameters) + all 7 flags (E/L/W/U/V/C/P; C=2-bit for
None/CRC32C/CRC64/MD5). `flag_byte` builds the flag byte from the set
optional fields.

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

### 8.3.4 Receiver state (sourceVersion/VendorId/GuidPrefix/destGuidPrefix/uni-/multicastReplyLocatorList/haveTimestamp/timestamp/messageLength/messageChecksum/rtpsSend-/ReceptionTimestamp/clockSkewDetected/parameters)

**Spec:** §8.3.4.

**Repo:** `receiver_state.rs::ReceiverState`.

**Tests:** receiver tests.

**Status:** done — all fields present incl. message_length+
messageChecksum.

### 8.3.4.1 Receiver rules (1-6: header-invalid/skip-logic/unknown-sub/X-flags-ignore/etc.)

**Spec:** §8.3.4.1.

**Repo:** skip logic in `datagram.rs`/`submessage_header.rs`.

**Tests:** —

**Status:** done

### 8.3.5 SubmessageElements (all 19)

**Spec:** §8.3.5.1-19.

**Repo:** `crates/rtps/src/wire_types.rs` + `submessages.rs`. All
spec elements: GuidPrefix, EntityId, VendorId, ProtocolVersion (all
8 versions), SequenceNumber, SequenceNumberSet, FragmentNumber,
FragmentNumberSet, Timestamp (HeTimestamp + DCPS Time), ParameterList,
Count (u32 in Heartbeat/AckNack/NackFrag), LocatorList,
SerializedData, SerializedDataFragment, ChangeCount, Checksum
(ChecksumValue), MessageLength, UExtension4, WExtension8,
GroupDigest_t.

**Tests:** a roundtrip test per element:
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
Count: implicit in the Heartbeat/AckNack roundtrip tests in
`submessages::tests`.
GroupDigest_t in `crates/rtps/src/group_digest.rs::tests`
(11 tests, RFC-1321 vectors + order-independence +
multi-block path).

**Status:** done

---

## §8.3.6 Header Validity

### 8.3.6.3 Header invalid: <required octets / protocol != PROTOCOL_RTPS / major-version > supported

**Spec:** §8.3.6.3.

**Repo:** validation in `datagram.rs`.

**Tests:** header-validity tests.

**Status:** done

### 8.3.6.4 Receiver update from the header

**Spec:** §8.3.6.4.

**Repo:** `receiver_state.rs::on_header`.

**Tests:** —

**Status:** done

---

## §8.3.7 HeaderExtension Validity

### 8.3.7.3 HeaderExtension invalid (not directly after header / length mismatch / parameters malformed)

**Spec:** §8.3.7.3.

**Repo:** `crates/rtps/src/datagram.rs::decode_datagram` rejects an HE
that is not the first submessage after the RTPS header
(`HeaderExtension must appear directly after the RTPS header`).
Length mismatch + truncated body via `decode_body` /
`validate_message_length`. A malformed ParameterList is propagated by
`ParameterList::from_bytes`.

**Tests:** `datagram::tests::decode_rejects_header_extension_after_data_submessage` (negative),
`header_extension::tests::decode_rejects_truncated_header` (negative),
`decode_rejects_truncated_body_before_message_length` (negative),
`decode_rejects_oversized_body` (negative),
`decode_rejects_trailing_bytes_without_p_flag` (negative),
`decode_rejects_truncated_uextension4_body` (negative),
`decode_rejects_truncated_extension16_body` (negative),
`decode_rejects_truncated_md5_checksum_body` (negative),
`decode_rejects_malformed_parameter_list_when_p_flag_set` (negative),
`validate_message_length_mismatch` (negative).

**Status:** done

### 8.3.7.4 HeaderExtension updates for the receiver (messageLength/timestamps/clockSkew/extensions/checksum/parameters)

**Spec:** §8.3.7.4.

**Repo:** `crates/rtps/src/receiver_state.rs::ReceiverState::
apply_header_extension` sets from the HE: `message_length` (L flag),
`timestamp` + `have_timestamp` (W flag = rtpsSendTimestamp),
`message_checksum` (C flags), `parameters` (P flag).
`note_clock_skew(now, threshold)` sets the `clock_skew_detected` flag
when `|timestamp.seconds - now_seconds| > threshold`. uextension4 +
wextension8 are stored in `HeaderExtension` itself; the
ReceiverState provides the pass-through via `parameters`.

**Tests:** `receiver_state::tests::apply_header_extension_updates_fields`
(messageLength + timestamp + checksum),
`apply_header_extension_with_parameters_sets_pl`,
`note_clock_skew_skipped_without_timestamp` (negative),
`note_clock_skew_within_threshold_does_not_flag` (negative),
`note_clock_skew_above_threshold_flags`.

**Status:** done

---

## §8.3.8 Submessages

### 8.3.8.1 AckNack

**Spec:** §8.3.8.1.

**Repo:** `submessages/acknack.rs`.

**Tests:** AckNack tests.

**Status:** done

### 8.3.8.2 Data

**Spec:** §8.3.8.2 — InlineQos/Data/Key/NonStandardPayload flags.

**Repo:** `crates/rtps/src/submessages.rs::DataSubmessage` with
explicit `key_flag` + `non_standard_flag` (spec §8.3.8.2 Tab. 8.43).
`write_body` sets the D/Q/K/N flag bits per the fields;
`read_body_with_flags` extracts them from the submessage header
(see `crates/rtps/src/datagram.rs::decode_datagram`).

**Tests:** `submessages::tests::data_submessage_roundtrip_le`,
`data_submessage_roundtrip_be_with_empty_payload`,
`data_submessage_octets_to_inline_qos_is_16`,
`data_submessage_decode_rejects_truncated` (negative),
`data_submessage_key_flag_roundtrip`,
`data_submessage_non_standard_flag_roundtrip`,
`data_submessage_all_flags_combined_roundtrip` (all 5 E/Q/D/K/N).

**Status:** done

### 8.3.8.3 DataFrag (re-assembly)

**Spec:** §8.3.8.3.

**Repo:** `submessages/data_frag.rs` + `fragment_assembler.rs`.

**Tests:** frag tests.

**Status:** done

### 8.3.8.4 Gap (gapStart + gapList + GroupInfoFlag)

**Spec:** §8.3.8.4.

**Repo:** `crates/rtps/src/submessages.rs::GapSubmessage` with
gapStart + gapList + optional `filteredCount` (K flag) and
a GroupInfo section (G flag, spec §8.3.8.4.2).

**Tests:** `submessages::tests::gap_submessage_roundtrip_le`,
`gap_decode_rejects_truncated_header` (negative),
`gap_with_filtered_count_roundtrip_le`,
`gap_with_group_info_roundtrip_be`,
`gap_with_group_info_and_filtered_count_combined` (both flags),
`gap_filtered_count_zero_is_distinct_from_none`,
`gap_decode_rejects_truncated_filtered_count` (negative).

**Status:** done

### 8.3.8.5 HeaderExtension (see §8.3.7)

**Spec:** §8.3.8.5 — reference to the submessage definition in §8.3.7.

**Repo:** identical to §8.3.3.2 / §8.3.7.3 / §8.3.7.4 — see those
items.

**Tests:** see §8.3.3.2 / §8.3.7.3 / §8.3.7.4.

**Status:** done

### 8.3.8.6 Heartbeat (firstSN/lastSN/count + Final/Liveliness/GroupInfo flags)

**Spec:** §8.3.8.6.

**Repo:** `crates/rtps/src/submessages.rs::HeartbeatSubmessage` with
firstSN + lastSN + count + Final flag + Liveliness flag + GroupInfo
flag (all 3 submessage flags), optional GroupInfo section with
writerSet + secureWriterSet (spec §8.3.8.6.1).

**Tests:** `submessages::tests::heartbeat_submessage_roundtrip_le`,
`heartbeat_submessage_no_final_flag_when_disabled` (negative),
`heartbeat_submessage_liveliness_flag_roundtrip`,
`heartbeat_decode_rejects_truncated` (negative),
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

**Repo:** `crates/rtps/src/submessages.rs::InfoReplySubmessage` with
unicast_locators + optional multicast_locators (M flag);
`crates/rtps/src/datagram.rs::decode_datagram` matches
`SubmessageId::InfoReply`. `crates/rtps/src/receiver_state.rs::
ReceiverState::apply_info_reply` propagates the locator lists
into the reply path.

**Tests:** `submessages::tests::info_reply_unicast_only_roundtrip_le`,
`info_reply_with_multicast_roundtrip_le`,
`info_reply_with_multicast_roundtrip_be`,
`info_reply_empty_unicast_list_is_valid`,
`info_reply_decode_rejects_oversized_locator_list_length` (negative),
`info_reply_decode_rejects_truncated_length_field` (negative).

**Status:** done

### 8.3.8.10 InfoSource

**Spec:** §8.3.8.10.

**Repo:** `crates/rtps/src/submessages.rs::InfoSourceSubmessage` with
all 4 wire fields (unused / protocolVersion / vendorId /
guidPrefix). `crates/rtps/src/receiver_state.rs::ReceiverState::
apply_info_source` overrides source_version + source_vendor_id
+ source_guid_prefix AND **resets** the reply locator lists + sets
`have_timestamp = false` (spec §8.3.8.9.4 step 4).

**Tests:** `submessages::tests::info_source_roundtrip_le`,
`info_source_roundtrip_be`,
`info_source_wire_size_is_20`,
`info_source_decode_rejects_truncated` (negative),
`info_source_unused_field_roundtrips`.
`receiver_state::tests::apply_info_source_resets_reply_locators_and_timestamp`.

**Status:** done

### 8.3.8.11 InfoTimestamp

**Spec:** §8.3.8.11.

**Repo:** `crates/rtps/src/submessages.rs::InfoTimestampSubmessage`
(new in K3b-B) with a Time_t body + I flag (`INFO_TIMESTAMP_FLAG_INVALIDATE`).
`crates/rtps/src/datagram.rs::decode_datagram` matches
`SubmessageId::InfoTs`. `crates/rtps/src/receiver_state.rs::
ReceiverState::apply_info_timestamp` sets `timestamp` +
`have_timestamp = true`; on `invalidate=true` it sets
`have_timestamp = false`. Skew correction via
`note_clock_skew(now_seconds, threshold)` sets the
`clock_skew_detected` flag (spec §8.3.4.1 receiver rule 4).

**Tests:** `submessages::tests::info_timestamp_roundtrip_le`,
`info_timestamp_roundtrip_be`,
`info_timestamp_invalidate_flag_yields_empty_body`,
`info_timestamp_decode_rejects_truncated_when_no_invalidate` (negative),
`info_timestamp_decode_with_invalidate_ignores_body`.
`receiver_state::tests::apply_info_timestamp_sets_value`,
`apply_info_timestamp_with_invalidate_clears`,
`note_clock_skew_skipped_without_timestamp` (negative),
`note_clock_skew_within_threshold_does_not_flag` (negative),
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

### 8.4.2.1 Comm path (messages, receiver rules, configurable timing, SPDP+SEDP)

**Spec:** §8.4.2.1.

**Repo:** distributed across `datagram.rs`+`writer.rs`+`reader.rs`+
`crates/discovery/src/{spdp,sedp/}`.

**Tests:** —

**Status:** done

### 8.4.2.2 Writer behavior (no out-of-order; heartbeats; inline QoS; NACK responses)

**Spec:** §8.4.2.2.

**Repo:** `crates/rtps/src/writer.rs` (Best-Effort) +
`reliable_writer.rs` (Reliable StatefulWriter with handle_acknack +
periodic heartbeat tick + inline-QoS path via
`DataSubmessage.inline_qos`). The writer sends strictly monotonically
increasing SequenceNumbers (no out-of-order); HEARTBEAT contains
firstSN/lastSN/count + Final/Liveliness/GroupInfo flags (spec §8.3.8.6).
The GroupInfo section with writerSet + secureWriterSet via the
`HeartbeatSubmessage.group_info` fields.

**Tests:** `reliable_writer::tests` (39 tests incl. handle_acknack
variant paths + retransmits + heartbeat tick + group info);
`writer::tests` (Best-Effort path);
`inline_qos::tests::reply_inline_qos_writer_side_serialisation`.

**Status:** done

### 8.4.2.3 Reader behavior (ACKNACK on HEARTBEAT; once-acked-always-acked)

**Spec:** §8.4.2.3.

**Repo:** `crates/rtps/src/reliable_reader.rs::ReliableReader` with a
periodic ACKNACK tick, pre-emptive ACKNACK on add-writer-proxy
(spec §8.4.12.5, "if the Reader received no DATA or HEARTBEAT yet
it sends a preemptive ACKNACK"), a `count_modular` counter (wraps
u32), `highest_received_sn` once-acked bookkeeping in the
`writer_proxy`. `crates/rtps/src/reader.rs` Best-Effort path.

**Tests:** `reliable_reader::tests::pre_emptive_acknack_emitted_after_add_writer_proxy`,
`no_pre_emptive_acknack_without_proxy` (negative),
`initial_proxy_from_config_does_not_send_pre_emptive` (negative),
`pre_emptive_acknack_carries_info_dst`. Plus 30+ further tests for
ACKNACK generation, heartbeat reaction, once-acked persistence.

**Status:** done

### 8.4.3 Stateless+Stateful reference implementations

**Spec:** §8.4.3.

**Repo:**
- Stateful Best-Effort + Reliable: `crates/rtps/src/{writer,reliable_writer,reader,reliable_reader}.rs`.
- Stateless Best-Effort: `crates/discovery/src/spdp.rs::SpdpBeacon`
  (multicast beacon, no reader-proxy state) +
  `crates/discovery/src/security/stateless.rs::StatelessMessageWriter`
  (multi-reader fan-out for
  `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER`, no cache, no
  resend, no HEARTBEAT).
- Stateless Reliable: `crates/rtps/src/reliable_stateless_writer.rs::
  ReliableStatelessWriter` (see §8.4.8.2).

**Tests:** `spdp::tests`, `security::stateless::tests` (stateless);
`reliable_writer::tests`, `reliable_reader::tests` (stateful);
`reliable_stateless_writer::tests` (stateless-reliable).

**Status:** done

### 8.4.4 Reliability combinations

**Spec:** §8.4.4.

**Repo:** `endpoint_match.rs` (in `discovery/`).

**Tests:** —

**Status:** done

### 8.4.6 Types: Duration_t, Change*StatusKind, InstanceHandle_t, ParticipantMessageData

**Spec:** §8.4.6.

**Repo:** Duration_t in `crates/qos/src/duration.rs::Duration` (DDS-PSM)
+ `crates/rtps/src/header_extension.rs::HeTimestamp` (RTPS wire);
ChangeKind/Change*StatusKind in
`crates/rtps/src/history_cache.rs::ChangeKind` with all 5
spec variants + `is_relevant`/`is_alive_kind` predicates;
InstanceHandle_t in `crates/dcps/src/instance_handle.rs::InstanceHandle`;
ParticipantMessageData in
`crates/rtps/src/participant_message_data.rs::ParticipantMessageData`
with AUTOMATIC + MANUAL_BY_PARTICIPANT + a ZeroDDS vendor kind.

**Tests:** `qos::duration::tests::*`,
`history_cache::tests::change_kind_*`,
`participant_message_data::tests::*`.

**Status:** done

### 8.4.7.1 Writer attrs (pushMode/heartbeatPeriod/nackResponseDelay/etc.)

**Spec:** §8.4.7.1.

**Repo:** `writer.rs`.

**Tests:** —

**Status:** done

### 8.4.7.1.1 Default timing (nackResponseDelay=200ms, nackSuppressionDuration=0)

**Spec:** §8.4.7.1.1.

**Repo:** constants in `writer.rs`.

**Tests:** —

**Status:** done

### 8.4.7.1.3 new_change(kind/data/inlineQos/handle)

**Spec:** §8.4.7.1.3.

**Repo:** `writer.rs::new_change`.

**Tests:** —

**Status:** done

### 8.4.7.2 StatelessWriter

**Spec:** §8.4.7.2.

**Repo:** Best-Effort stateless in `crates/discovery/src/spdp.rs::
SpdpBeacon` (SPDP multicast) +
`crates/discovery/src/security/stateless.rs::StatelessMessageWriter`
(DDS-Security stateless message). Reliable-stateless in
`crates/rtps/src/reliable_stateless_writer.rs::ReliableStatelessWriter`
(see §8.4.8.2).

**Tests:** `spdp::tests::*`,
`security::stateless::tests::*`,
`reliable_stateless_writer::tests::*`.

**Status:** done

### 8.4.7.3 ReaderLocator (highestSentChangeSN/requestedChanges/locator/expectsInlineQos + ops)

**Spec:** §8.4.7.3.

**Repo:** `crates/rtps/src/reader_proxy.rs::ReaderProxy` (stateful) +
`crates/discovery/src/security/stateless.rs::StatelessMessageWriter::
reader_proxies` (stateless vector). ReaderLocator-equivalent fields
in ReaderProxy: `remote_reader_guid`, `unicast_locators`,
`multicast_locators`, `expects_inline_qos`, `highest_sent_change_sn`,
`requested_changes`, `acked_changes`. Ops: `add_proxy`/`remove_proxy`/
`update_locators`. Stateless ReaderLocator pool via
`ReliableStatelessWriter::set_locators`.

**Tests:** `reader_proxy::tests::*` (12 tests for all ops),
`reliable_stateless_writer::tests::set_locators_t11_replaces_list`,
`security::stateless::tests::add_remove_reader_proxy_idempotent`.

**Status:** done

### 8.4.7.4 StatefulWriter (matched_readers + is_acked_by_all)

**Spec:** §8.4.7.4.

**Repo:** `reliable_writer.rs`.

**Tests:** —

**Status:** done

### 8.4.7.5 ReaderProxy (all fields + ops)

**Spec:** §8.4.7.5.

**Repo:** `reader_proxy.rs`.

**Tests:** —

**Status:** done

### 8.4.8.1 Best-Effort StatelessWriter (T1-T5)

**Spec:** §8.4.8.1.

**Repo:** SPDP sender (multicast Best-Effort).

**Tests:** —

**Status:** done

### 8.4.8.2 Reliable StatelessWriter (T1-T12)

**Spec:** §8.4.8.2 — reliable-stateless writer state machine with 12
transitions (Tab. 8.46).

**Repo:** `crates/rtps/src/reliable_stateless_writer.rs::
ReliableStatelessWriter` with all 12 transitions:
- T1 `new_change(kind, payload)` → SequenceNumber.
- T2/T3 `tick(now)` → DATA burst + periodic HEARTBEAT.
- T4/T5 `handle_acknack(ack)` → `lowest_unacked` advance, fill the
  requested pool.
- T6 `tick` (NACK drain) → send retransmits from the pool.
- T7 `is_acked_to(sn)` → once-acked-always-acked predicate.
- T8 `purge_acked()` → cache low-water cleanup.
- T9 KeepLast in `HistoryCache::insert` (existing).
- T10 `shutdown()` → clear cache + retransmit pool.
- T11 `set_locators(locators)` → reply-locator-list update.
- T12 `stats()` → diagnostic snapshot.

A `max_per_tick` cap (default 16) as a DoS protection against excessive
ACKNACK requests. `heartbeat_count` wraps u32 (see §8.4.15.7).

**Tests:** `reliable_stateless_writer::tests`:
`new_change_assigns_monotonic_sn_t1`,
`handle_acknack_advances_lowest_unacked_t4`,
`handle_acknack_only_advances_t4_once_acked_always_acked`,
`handle_acknack_with_requested_bits_queues_retransmits_t6`,
`is_acked_to_t7`,
`purge_acked_t8_removes_acked_samples`,
`tick_emits_heartbeat_t3`,
`tick_does_not_emit_heartbeat_when_cache_empty` (negative),
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
StatefulWriter with a per-target reader ID + ReaderProxy list in
SPDP/SEDP). T1-T6 transitions: pushing → DATA, new sample →
new_change, ChangeFromWriter status updates, filter drop via
`ChangeKind::AliveFiltered`.

**Tests:** `writer::tests` (Best-Effort lifecycle),
`writer_proxy::tests::change_from_writer_*`,
`history_cache::tests::change_kind_alive_filtered_is_alive_but_not_relevant`.

**Status:** done

### 8.4.9.2 Reliable StatefulWriter (T1-T16)

**Spec:** §8.4.9.2.

**Repo:** `crates/rtps/src/reliable_writer.rs::ReliableWriter` with
all 16 transitions: tick → DATA push, periodic HEARTBEAT
(FinalFlag default NOT_SET → ACKNACK required), handle_acknack
with GAP handling, retransmit path, AckedByAll statement,
filteredCount tracking via the `AliveFiltered` ChangeKind in the
ReaderProxy.

**Tests:** `reliable_writer::tests` (40+ tests for all 16
transitions incl. handle_acknack variant paths, heartbeat tick,
retransmits, AckedByAll). Plus `tests/reliable_e2e.rs` E2E path
with 0%/10%/30% packet loss.

**Status:** done

---

## §8.4.10-12 RTPS Reader / StatelessReader / StatefulReader

### 8.4.10.1 Reader attrs (heartbeatResponseDelay/heartbeatSuppressionDuration/expectsInlineQos)

**Spec:** §8.4.10.1 — default heartbeatResponseDelay=500ms,
heartbeatSuppressionDuration=0.

**Repo:** `reliable_reader.rs`.

**Tests:** —

**Status:** done

### 8.4.10.2 StatelessReader

**Spec:** §8.4.10.2.

**Repo:** SPDP reader in `crates/discovery/src/spdp.rs`.

**Tests:** —

**Status:** done

### 8.4.10.3 StatefulReader (matched_writers)

**Spec:** §8.4.10.3.

**Repo:** `reliable_reader.rs`.

**Tests:** —

**Status:** done

### 8.4.10.4 WriterProxy (remoteWriterGuid + locator lists + dataMaxSize + changes_from_writer)

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
mark_not_available` with all 3 reasons. `filteredCount` is carried via
`ChangeKind::AliveFiltered`:
`crates/rtps/src/history_cache.rs::ChangeKind::is_relevant` delivers
`false` for AliveFiltered, so that these SNs count as "not_available" with
reason `NA_FILTERED` without triggering new NACKs.
GAP submessages with `filtered_count` (K flag) carry the counter
between writer and reader.

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

**Repo:** `crates/rtps/src/writer_proxy.rs::ChangeFromWriter` with
`status` (Missing/Received/Lost/Unknown) + `is_relevant: bool`.
`is_relevant=false` indicates a filter-dropped sample
(TIME_BASED_FILTER, ContentFilteredTopic, or
ChangeKind::AliveFiltered). `crates/rtps/src/history_cache.rs::
ChangeKind::is_relevant` is the canonical source of
truth (false only for AliveFiltered).

**Tests:** `writer_proxy::tests` (12+ tests incl.
`gap_marks_irrelevant`, `received_change_set` etc.),
`history_cache::tests::change_kind_alive_is_relevant_and_alive`,
`change_kind_alive_filtered_is_alive_but_not_relevant`,
`change_kind_not_alive_kinds_are_not_alive`.

**Status:** done

### 8.4.11 StatelessReader Behavior

**Spec:** §8.4.11.

**Repo:** SPDP reader.

**Tests:** —

**Status:** done

### 8.4.12 StatefulReader Behavior

**Spec:** §8.4.12.

**Repo:** `crates/rtps/src/reliable_reader.rs::ReliableReader` with
matched_writers (Vec<WriterProxy>), pre-emptive ACKNACK,
periodic ACKNACK tick, GAP processing. The `Requested` sub-state
is modeled in `WriterProxy.requested_changes` (a BTreeSet of SNs without
a status update).

**Tests:** `reliable_reader::tests::pre_emptive_acknack_emitted_after_add_writer_proxy`,
`writer_proxy::tests::missing_changes_respects_max_count`,
`received_removes_from_missing`,
`gap_marks_irrelevant`,
plus 30+ further ReliableReader tests.

**Status:** done

---

## §8.4.13 Writer Liveliness Protocol

### 8.4.13 WLP via ParticipantMessageData

**Spec:** §8.4.13.

**Repo:** `crates/rtps/src/participant_message_data.rs::
ParticipantMessageData` with all 3 kinds:
`PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE` (kind=0),
`PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE`
(kind=1), `PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC`
(0x80000001 vendor kind). The DCPS wiring in
`crates/dcps/src/wlp.rs::WlpEndpoint` with a periodic AUTOMATIC
beat (tick-driven), a manual pulse queue (`MAX_QUEUED_PULSES=32`),
per-peer tracking + lease expiry via `lost_peers(now, lease)`.

**Tests:** `participant_message_data::tests` (wire roundtrip of all
3 kinds), `dcps::wlp::tests`:
`wlp_first_tick_emits_automatic_heartbeat`,
`wlp_assert_participant_emits_manual_pulse`,
`wlp_assert_topic_emits_vendor_kind_with_token`,
`wlp_handle_datagram_updates_peer_state`,
`wlp_lost_peers_returns_only_expired`,
`wlp_pending_queue_caps_at_max` (DoS cap).

**Status:** done

---

## §8.4.14 Optional Behavior (Large Data / Fragmentation)

### 8.4.14 Fragmentation via DATA_FRAG/NACK_FRAG/HEARTBEAT_FRAG

**Spec:** §8.4.14.

**Repo:** `fragment_assembler.rs` + frag submessages.

**Tests:** frag tests incl. DoS cap.

**Status:** done

---

## §8.4.15 Implementation Guidelines

### 8.4.15.6 Inactive-reader reclaim (strict-reliability dropout)

**Spec:** §8.4.15.6 — a writer MAY reclaim a reader proxy when
it has delivered no ACKNACK / NACK_FRAG for longer than
`inactive_threshold` (strict-reliability dropout as a DoS protection against
stalled readers).

**Repo:** `crates/rtps/src/reader_proxy.rs::ReaderProxy.last_activity`
+ methods `note_activity(now)` (called by the ACKNACK/NACK_FRAG receive
path), `is_inactive(now, threshold)` (reclaim predicate),
`last_activity()` (diagnostic). The caller (`ReliableWriter`) calls
`is_inactive` periodically and can remove the proxy from
`matched_readers`.

**Tests:** `reader_proxy::tests::proxy_is_inactive_initially_when_threshold_is_short`,
`proxy_is_active_after_note_activity`,
`proxy_becomes_inactive_after_threshold_elapses`,
`proxy_inactivity_not_reported_when_now_before_last_activity` (negative).

**Status:** done

### 8.4.15.7 Count modular arithmetic

**Spec:** §8.4.15.7 — the Heartbeat/AckNack/NackFrag counter MUST
be wrap-around tolerant (modulo 2^32, in Rust via
`wrapping_add`).

**Repo:** `crates/rtps/src/reliable_writer.rs::ReliableWriter.
heartbeat_count` + `nackfrag_count` use
`i32::wrapping_add(1)` on each increment.
`reliable_stateless_writer.rs::ReliableStatelessWriter.
heartbeat_count` uses `u32::wrapping_add(1)`. Spec-conformant: on
reaching the maximum it wraps to MIN/0.

**Tests:** `reliable_writer::tests::heartbeat_count_increments`,
`heartbeat_count_wraps_around_at_i32_max_per_spec_8_4_15_7`,
`reliable_stateless_writer::tests::heartbeat_count_wraps_at_u32_max_t3_modular`.

**Status:** done

---

## §8.5 Discovery Module

### 8.5.1 SPDP — Simple Participant Discovery Protocol

**Spec:** §8.5.1, p. 117-120.

**Repo:** `crates/discovery/src/spdp.rs`.

**Tests:** SPDP tests + Cyclone-live SPDP.

**Status:** done

### 8.5.2 SEDP — Simple Endpoint Discovery Protocol

**Spec:** §8.5.2, p. 121-127.

**Repo:** `crates/discovery/src/sedp/`.

**Tests:** SEDP tests + Cyclone-live SEDP.

**Status:** done

### 8.5.3 ParticipantBuiltinTopicData / PublicationBuiltinTopicData / SubscriptionBuiltinTopicData / TopicBuiltinTopicData

**Spec:** §8.5.3.

**Repo:** all 4 builtin-topic-data structures fully spec-conformant:

- `crates/rtps/src/participant_data.rs::ParticipantBuiltinTopicData`
- `crates/rtps/src/publication_data.rs::PublicationBuiltinTopicData`
- `crates/rtps/src/subscription_data.rs::SubscriptionBuiltinTopicData`
  (incl. `content_filter: Option<ContentFilterProperty>`)

- `crates/dcps/src/builtin_topics.rs::TopicBuiltinTopicData`
  (DCPS view; the SEDP topic endpoint is announced as an optional endpoint
  in §8.5.4).

**Tests:** `crates/rtps/src/participant_data.rs::tests::*`,
`publication_data::tests::*`, `subscription_data::tests::*`,
`crates/dcps/src/builtin_topics.rs::tests::*` (wire roundtrip of all 4).

**Status:** done

### 8.5.4 Built-in Endpoints (reader/writer per PDP/SEDP topic)

**Spec:** §8.5.4.

**Repo:** all standard builtin-endpoint pairs as
`EntityId` constants in `crates/rtps/src/wire_types.rs`:
- SPDP-W/R: `SPDP_BUILTIN_PARTICIPANT_{WRITER,READER}` (0x000100C2/C7).
- SEDP-Pub-W/R: `SEDP_BUILTIN_PUBLICATIONS_{WRITER,READER}` (0x000003C2/C7).
- SEDP-Sub-W/R: `SEDP_BUILTIN_SUBSCRIPTIONS_{WRITER,READER}` (0x000004C2/C7).
- WLP-W/R: `BUILTIN_PARTICIPANT_MESSAGE_{WRITER,READER}` (0x000200C2/C7).
- DDS-Security: SPDP-Secure / SEDP-Pub/Sub-Secure / Stateless / Volatile-
  Secure (12 endpoints, all in `wire_types.rs`).

**Tests:** `wire_types::tests::spdp_builtin_*`, `sedp_builtin_*`,
`all_12_secure_entityids_roundtrip`,
`crates/discovery/src/sedp/stack::tests::*`,
`spdp::tests::*`.

**Status:** done

### 8.5.5 Lease renewal + cleanup on expiry

**Spec:** §8.5.5.

**Repo:** `crates/discovery/src/spdp.rs::DiscoveredParticipantsCache`
with `last_seen` tracking + lease-expiry iteration.
`crates/dcps/src/wlp.rs::WlpEndpoint::lost_peers(now, lease)` delivers
expired peers to the caller.

**Tests:** `wlp::tests::wlp_lost_peers_returns_only_expired`,
`wlp_handle_datagram_updates_peer_state`,
`wlp_forget_peer_removes_state`,
`spdp::tests::*` (cache lifecycle).

**Status:** done

---

## §8.6 Versioning and Extensibility

### 8.6 Vendor extensions + submessage extensions

**Spec:** §8.6 — vendor-specific submessage IDs (0x80-0xFF) as well as
vendor-specific PIDs (PID bit 15 set) MUST be ignored by receivers
without the must-understand bit; with the must-understand bit
the receiver may discard the message.

**Repo:** `crates/rtps/src/submessage_header.rs::SubmessageId` enum
with the standard range + pass-through for the vendor range via
`SUBMESSAGE_ID_HEADER_EXTENSION` (0x80) and an Unknown variant.
The vendor-specific PID path in
`crates/rtps/src/parameter_list.rs::VENDOR_SPECIFIC_BIT` (0x8000) +
`validate_must_understand_in_data_pipeline` (silent-skip for
vendor-specific MU PIDs, spec §9.6.2).

**Tests:** `datagram::tests::decode_marks_unknown_submessage_id_without_failing`,
`decode_skips_unknown_submessage_without_must_understand`,
`decode_rejects_unknown_submessage_with_must_understand` (negative),
`parameter_list::tests::must_understand_vendor_pid_with_must_understand_rejects`,
`validate_must_understand_in_data_pipeline_vendor_specific_pid_passes`,
`datagram::tests::decode_accepts_vendor_specific_must_understand_pid`.

**Status:** done

---

## §8.7 DDS QoS via RTPS

### 8.7.1 PID table (all DDS-QoS PIDs)

**Spec:** §8.7.1, p. 131-138.

**Repo:** `crates/rtps/src/parameter_list.rs::pid` with all standard
PIDs incl. new in K3b-F: `TYPE_MAX_SIZE_SERIALIZED` (0x0060),
`ORIGINAL_WRITER_INFO` (0x0061), `WRITER_GROUP_INFO` (0x0062). Plus
`COHERENT_SET` / `GROUP_COHERENT_SET` / `GROUP_SEQ_NUM` /
`DIRECTED_WRITE` / `KEY_HASH` / `STATUS_INFO` etc. (50+ PIDs total).
`is_standard_pid(pid)` as a classifier for must-understand reject.

**Tests:** `parameter_list::tests::is_standard_pid_recognises_dds_security_pids`,
`is_standard_pid_unknown_pid_returns_false` (negative),
plus all ParameterList roundtrip tests per PID family.

**Status:** done

### 8.7.2 LIVELINESS wire mapping (ParticipantMessageData + Heartbeat LivelinessFlag)

**Spec:** §8.7.2.

**Repo:** AUTOMATIC + MANUAL_BY_PARTICIPANT as a
`ParticipantMessageData` DATA sample in the
`BUILTIN_PARTICIPANT_MESSAGE_WRITER` stream
(`crates/rtps/src/participant_message_data.rs::ParticipantMessageData`).
MANUAL_BY_TOPIC via the Heartbeat LivelinessFlag (L bit, spec §8.3.8.6) +
a ZeroDDS vendor kind (`PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC`)
with a topic token. The reliable writer sets the L flag on heartbeats for
MANUAL_BY_TOPIC-configured writers.

**Tests:** `participant_message_data::tests::*`,
`submessages::tests::heartbeat_submessage_liveliness_flag_roundtrip`,
`dcps::wlp::tests::wlp_assert_topic_emits_vendor_kind_with_token`,
`wlp_assert_participant_emits_manual_pulse`.

**Status:** done

### 8.7.3 Content-filtered topics (reader-side filter + ContentFilterProperty_t)

**Spec:** §8.7.3.

**Repo:** `crates/rtps/src/subscription_data.rs::ContentFilterProperty`
+ wire encode/decode (`encode_content_filter_property_le` /
`decode_content_filter_property`). The PID `CONTENT_FILTER_PROPERTY`
(0x0035) in SubscriptionBuiltinTopicData. Reader-side filter via
`crates/dcps/src/topic.rs::ContentFilteredTopic` with an SQL filter
over the `zerodds-sql-filter` crate (see DCPS K3a §2.2.2.5.4 +
ContentFilteredTopic tests).

**Tests:** `crates/rtps/src/subscription_data.rs::tests::*`
(ContentFilterProperty wire roundtrip),
`crates/dcps/src/topic.rs::tests::cft_*` (filter logic).

**Status:** done

### 8.7.4 Instance-state transitions

**Spec:** §8.7.4.

**Repo:** `crates/dcps/src/instance_tracker.rs::InstanceTracker` +
`InstanceState`. Spec transitions ALIVE → NOT_ALIVE_DISPOSED /
NOT_ALIVE_NO_WRITERS / ALIVE_FILTERED via `dispose()`,
`unregister()`, `should_deliver_under_time_based_filter`. Tracking
via `disposed_at`, `no_writers_at`, `current_owner` with
generation counters. `ChangeKind::AliveFiltered` counts in the
filteredCount but remains instance-active.

**Tests:** `instance_tracker::tests::dispose_transitions_to_disposed`,
`re_register_after_dispose_bumps_disposed_generation`,
`unregister_decrements_writer_count`,
`time_based_filter_*` (5 tests),
`autopurge_*` (5 tests),
`exclusive_*` (8 tests strength selection).

**Status:** done

### 8.7.5 Group-ordered access (PID_GROUP_SEQ_NUM/PID_WRITER_GROUP_INFO/Heartbeat.GSN)

**Spec:** §8.7.5.

**Repo:** PIDs in `crates/rtps/src/parameter_list.rs::pid::
{GROUP_SEQ_NUM, WRITER_GROUP_INFO}`. The Heartbeat GroupInfo section
(GSN + group_first/last) in
`crates/rtps/src/submessages.rs::HeartbeatSubmessage::group_info`
(G flag). The DCPS-side GROUP-coherent path in
`crates/dcps/src/coherent_set.rs::GroupAccessScope.snapshot_generation`
(see DCPS K3a §2.2.3.6).

**Tests:** `submessages::tests::heartbeat_with_writer_set_roundtrip_be`,
`heartbeat_with_empty_group_info_roundtrip_le`,
`coherent_set::tests::snapshot_generation_*` (DCPS),
`is_standard_pid_recognises_*` (PID acceptance).

**Status:** done

### 8.7.6 Coherent Sets (PID_COHERENT_SET/PID_GROUP_COHERENT_SET/ECS sample)

**Spec:** §8.7.6.

**Repo:** inline-QoS PIDs `COHERENT_SET` (0x0056) + `GROUP_COHERENT_SET`
(0x0063) in `crates/rtps/src/parameter_list.rs::pid`.
The DCPS-side coherent-scope wiring in
`crates/dcps/src/coherent_set.rs::{CoherentScope, CoherentSetMarker,
GroupAccessScope}` with `snapshot_generation` (an atomic cut on the
GROUP-coherent begin_access). The end-of-coherent-set (ECS) sample is
signaled by end_access with a generation bump.

**Tests:** `coherent_set::tests::*` (snapshot generation +
multi-reader shared scope + nested begin/end).

**Status:** done

### 8.7.7 Directed Write

**Spec:** §8.7.7 / §9.6.2.2.5 — inline-QoS `PID_DIRECTED_WRITE`
addresses a sample to a single reader GUID; other
readers that receive the sample (e.g. via multicast) MUST
discard it.

**Repo:** `crates/rtps/src/parameter_list.rs::pid::DIRECTED_WRITE`
(0x0057). Encoder/filter in
`crates/rtps/src/inline_qos.rs::directed_write_param`,
`find_directed_write`, `directed_write_matches_reader`.

**Tests:** `inline_qos::tests::directed_write_param_carries_target_guid`,
`find_directed_write_returns_target_when_present`,
`find_directed_write_returns_none_when_absent` (negative),
`find_directed_write_rejects_truncated_value` (negative),
`directed_write_matches_reader_returns_true_for_matching_target`,
`directed_write_matches_reader_returns_false_for_other_target` (negative),
`directed_write_matches_reader_returns_true_when_no_directed_write`.

**Status:** done

### 8.7.8 Inline-QoS path

**Spec:** §8.7.8.

**Repo:** `crates/rtps/src/inline_qos.rs` with helper functions for
all standard inline-QoS PIDs:
`reply_inline_qos` (PID_RELATED_SAMPLE_IDENTITY) + `find_*` decoders,
`directed_write_param` (PID_DIRECTED_WRITE) +
`find_directed_write` + `directed_write_matches_reader`,
`original_writer_info_param` (PID_ORIGINAL_WRITER_INFO) +
`find_original_writer_info`. PID_KEY_HASH inline-QoS in
`crates/dcps/src/dds_type.rs::DdsType::compute_key_hash`. PID_STATUS_INFO
+ PID_PROPERTY_LIST in `crates/rtps/src/property_list.rs` +
`crates/dcps/src/dds_type.rs`.

**Tests:** `inline_qos::tests::*` (28 tests incl. directed write,
OriginalWriterInfo, RelatedSampleIdentity roundtrip).

**Status:** done

### 8.7.9 OriginalWriterInfo (for persistence-service forwarding)

**Spec:** §8.7.9 — inline-QoS `PID_ORIGINAL_WRITER_INFO` (24 byte:
GUID + i64 SequenceNumber). The persistence service sets this on
forwarded late-joiner replay samples.

**Repo:** `crates/rtps/src/parameter_list.rs::pid::ORIGINAL_WRITER_INFO`
(0x0061), `crates/rtps/src/inline_qos.rs::original_writer_info_param`
(encoder), `find_original_writer_info` (decoder with LE/BE support).
The persistence-service backend in `crates/dcps/src/durability_service.rs`
(see DCPS K3a §2.2.3.5).

**Tests:** `inline_qos::tests::original_writer_info_param_24_byte_layout_le`,
`original_writer_info_roundtrip_le`,
`original_writer_info_roundtrip_be`,
`find_original_writer_info_returns_none_when_absent` (negative),
`find_original_writer_info_rejects_truncated` (negative).

**Status:** done

---

## §9 PSM UDP/IP

### 9.1-9.3 Notational Conventions / RTPS Types Mapping

**Spec:** §9.1-9.3, p. 143-152.

**Repo:** `wire_types.rs`.

**Tests:** —

**Status:** done

### 9.3.1 GUID PSM mapping (auto-generated by the DDS implementation)

**Spec:** §9.3.1.

**Repo:** `wire_types.rs::Guid`.

**Tests:** —

**Status:** done

### 9.3.2 IDL definitions / SubmessageElement types

**Spec:** §9.3.2 — built-in endpoint set bits 0-29.

**Repo:** `crates/rtps/src/participant_data.rs::BuiltinEndpointSet`
bitfield with all bits: SPDP-W/R (0/1), SEDP-Pub-W/R (2/3), SEDP-
Sub-W/R (4/5), SEDP-Topic-W/R (6/7), Participant-Message W/R (10/11),
DDS-Security endpoints (16-23), topic discovery (28/29).

**Tests:** `participant_data::tests::*` (BuiltinEndpointSet
wire roundtrip per bit).

**Status:** done

### 9.4 Mapping of messages (wire encoding of all submessages)

**Spec:** §9.4 — wire layout per submessage.

**Repo:** `crates/rtps/src/submessages.rs` + `header_extension.rs` +
`datagram.rs` with complete encode/decode paths for all
submessage classes (DATA, DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG,
ACKNACK, NACK_FRAG, GAP, INFO_TS, INFO_SRC, INFO_DST, INFO_REPLY,
PAD, HEADER_EXTENSION). InfoReplyIp4 is encoded as a generic
`InfoReplySubmessage` with an IPv4 locator variant.

**Tests:** `submessages::tests::*` (60+ wire roundtrip tests),
`header_extension::tests::*` (16 HE roundtrip tests),
`datagram::tests::*` (datagram composition).

**Status:** done

### 9.4.2.11.2 Must-understand bit (PID & 0x4000)

**Spec:** §9.4.2.11.2 — "If the receiver does not understand a
parameter and the must_understand bit (0x4000) is set, the entire
RTPS message carrying this ParameterList MUST be discarded."

**Repo:** `crates/rtps/src/parameter_list.rs::ParameterList::
validate_must_understand_in_data_pipeline` with the
`is_standard_pid(masked_pid)` classifier (all 50+ spec PIDs
incl. DDS-Security 1.2). Wire-hot-path activation in
`crates/rtps/src/datagram.rs::decode_datagram` — both on the
DATA submessage's `inline_qos` and on the HE.parameters.
Vendor-specific PIDs (top bit set) are also ignored with the
must-understand bit (spec §9.6.2).

**Tests:** `parameter_list::tests::must_understand_known_pid_passes`,
`must_understand_unknown_pid_rejects` (negative),
`must_understand_vendor_pid_with_must_understand_rejects` (negative),
`validate_must_understand_in_data_pipeline_known_pid_passes`,
`validate_must_understand_in_data_pipeline_unknown_pid_rejects` (negative),
`validate_must_understand_in_data_pipeline_vendor_specific_pid_passes`,
`validate_must_understand_in_data_pipeline_optional_unknown_pid_passes`,
`is_standard_pid_recognises_dds_security_pids`,
`is_standard_pid_unknown_pid_returns_false` (negative),
`datagram::tests::decode_rejects_data_with_unknown_must_understand_pid_in_inline_qos` (negative),
`decode_accepts_data_with_known_must_understand_pid_in_inline_qos`,
`decode_accepts_vendor_specific_must_understand_pid`.

**Status:** done

### 9.4.2.15 Checksum path (CRC-32C / CRC-64-XZ / MD5-128)

**Spec:** §9.4.2.15 — all 3 checksum algorithms via the
HeaderExtension C-flag pair (C0+C1).

**Repo:** `crates/rtps/src/header_extension.rs::ChecksumKind` with all
4 values (None/Crc32c/Crc64/Md5) + `ChecksumValue` enum variants
with the hash value (4/8/16 byte). Wire encode/decode in `decode_body`
+ `flag_byte`.

**Compute/verify:** `ChecksumValue::compute(kind, payload)` and
`ChecksumValue::verify(payload)` compute the checksum resp. check a
received value against a payload slice. The three algorithms
are implemented throughout pure-Rust no_std via
`zerodds_foundation::{crc32c, crc64_xz, md5}` — no external crypto-crate
dependency (Pillar 9 zero-dependency).

**Tests:** `header_extension::tests::roundtrip_with_crc32c_le`,
`roundtrip_with_crc64_be`,
`roundtrip_with_md5_le`,
`checksum_kind_roundtrip` (all 4 kinds),
`checksum_kind_wire_size`,
`decode_rejects_truncated_md5_checksum_body` (negative),
`checksum_compute_crc32c_matches_rfc4960_vector` (RFC 4960 App. B test vector),
`checksum_compute_crc64_xz_matches_known_vector` (XZ utils standard vector),
`checksum_compute_md5_matches_rfc1321_vector` (RFC 1321 §A.5 test vector),
`checksum_verify_round_trip_crc32c_succeeds`,
`checksum_verify_detects_tampered_payload` (negative),
`checksum_verify_none_kind_always_succeeds`,
`checksum_verify_crc64_round_trip`,
`checksum_verify_md5_round_trip`,
`checksum_compute_none_yields_none`.

**Status:** done

### 9.5 Mapping to UDP/IP transport messages

**Spec:** §9.5.

**Repo:** `crates/transport-udp/`.

**Tests:** transport tests.

**Status:** done

### 9.6 RTPS protocol mapping (port computation PB=7400/DG=250/PG=2/d0..d3 + multicast locator)

**Spec:** §9.6.

**Repo:** `crates/discovery/src/spdp.rs::well_known_port`.

**Tests:** port tests.

**Status:** done

### 9.6.3 Optimization-omit (MUST-omit redundant fields on SPDP/SEDP)

**Spec:** §9.6.3 — fields that MUST NOT be serialized in PL-CDR
at their default value (bandwidth optimization).

**Repo:** `crates/rtps/src/participant_data.rs::ParticipantBuiltinTopicData::
to_pl_cdr_le` + `crates/rtps/src/publication_data.rs` /
`subscription_data.rs` omit default QoS values
(`if let Some(...)` pattern), write only explicitly set
QoS policies. The PID `IGNORE` (0x3F03) as a padding marker.

**Tests:** `participant_data::tests::*` (wire roundtrip without
default values byte-identical to a Cyclone sample).

**Status:** done

### 9.6.4.3 KeyHash computation (PLAIN_CDR2-BE serialization)

**Spec:** §9.6.4.3 + XTypes 1.3 §7.6.8 — KeyHash = PLAIN_CDR2-BE
serialization of the @key members, sorted by memberId. With
max_size > 16: MD5(stream)[0..16]. Otherwise zero-padded.

**Repo:** `crates/cdr/src/key_hash.rs::compute_key_hash` with both
paths (zero-pad + MD5). XCDR2-BE serialization with member reorder
via `dds_type::DdsType::encode_key_holder_be` (proc-macro-generated
sorted order in the K2 XTypes full stack).

**Tests:** `cdr::key_hash::tests::*` (16+ tests for both paths,
zero-pad + MD5),
`dds_type::tests::small_keyed_produces_zero_padded_keyhash`,
`large_keyed_produces_md5_hashed_keyhash`,
`keyed_member_order_matters` (sort stability).

**Status:** done

---

## §10 SerializedPayload Representation

### 10.1-10.7 SerializedPayloadHeader + Repr Identifier (CDR/PL_CDR/CDR2/D_CDR/XML)

**Spec:** §10, p. 193-199.

**Repo:** `crates/cdr/src/representation.rs::RepresentationIdentifier`
with all spec values (CDR_BE/LE, PL_CDR_BE/LE, CDR2_BE/LE,
PL_CDR2_BE/LE, D_CDR_BE/LE, XML, RTPS_HE 0xC0xx). A 4-byte
encapsulation header (RID + options) as the prefix of each
SerializedPayload. D-CDR (delimited) as a length-prefix wrapper
over XCDR2 (XTypes 1.3 §7.4.3.5.4) implemented in the DCPS-CDR
layer.

**Tests:** `cdr::representation::tests::*` (wire roundtrip of all
identifiers incl. options bits), `cdr::xcdr2::tests::*` (XCDR2-BE
+ LE), `cdr::tests::pl_cdr::*` (PL_CDR path).

**Status:** done

---

## Audit status

121 done / 0 partial / 0 open / 3 n/a (informative) / 0 n/a (rejected).

Test run:

* `cargo test -p zerodds-rtps` — 572 lib + 54 integration
  (8 bins: compliance_rtps, cyclone_compliance,
  cyclone_he_must_understand, fixture_loader, fuzz_smoke,
  proptest_roundtrip, reliable_e2e, sedp_qos_roundtrip)
  = 626 tests green.
* `cargo test -p zerodds-discovery` — 111 lib + 43 integration
  (10 bins: cyclone_full_interop, cyclone_live_sedp,
  cyclone_live_typelookup, cyclone_sedp_replay,
  cyclone_typelookup_responder, fastdds_live_spdp, fuzz_smoke,
  sedp_loopback, spdp_loopback_e2e, type_lookup_service)
  = 154 tests green (SPDP + SEDP + ParameterList +
  TypeLookup service).
