# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Fixed
- `ReliableStreamState::recv_data` accepts the sample at `expected_seq` when the receiver buffer is full; it was refused with the others, so a full buffer never emptied (DDS-XRCE 1.0 §8.3.2.3: no message is lost on a reliable stream).

### Added
- `StatusAgentPayload::result_and_position`: reads the `ResultStatus` of a `STATUS_AGENT` payload when there is one in front of the `AGENT_Representation` (the text of §8.3.5.5.2, the eProsima agent) and a success when the payload is the `AGENT_Representation` alone (the IDL of Annex A).
- `encoding::SampleInfoForm` (`AnnexA` default, `Flat`), `set_sample_info_form`, `with_sample_info_form`; `SampleInfo` follows Annex A (`state` and a `SampleInfoDetail` union by `SampleInfoFormat`; written as `FORMAT_SEQN_TIMS`, which has the 12 octets of the flat struct of §7.7.1; read by its discriminator, 2, 8 or 12 octets) and `Flat` the struct with padding. `PendingSample` carries `state` and `source_time`.
- `ObjectVariant::decode_for_kind_at`: a binary representation followed by zero octets up to the next multiple of four and then exactly the id of the container is read as the Annex B.2.4 example prints it (a padding octet before the `publisher_id`).
- `encoding::RepresentationTimestamps` (`Absent` default, `Present`), `set_representation_timestamps`, `with_representation_timestamps`; `ClientRepresentation::timestamp` / `AgentRepresentation::timestamp` and `encode_form` / `decode_form` (DDS-XRCE 1.0 §7.8.2.1): the prose lists a `Time_t client_timestamp` / `agent_timestamp` after `xrce_vendor_id`, the IDL of Annex A and the bytes of Annex B.1 do not. The forms cannot be told apart by their bytes, so the form is a setting. `CreateClientPayload::into_submessage_form`.
- `encoding::UnannotatedTypes` (`Appendable`, default; `Final`), `set_unannotated_types`, `unannotated_types`, `with_unannotated_types`: the types of Annex A without `@extensibility` (`Time_t`, `ResultStatus`, `Property`, `TransportLocator*` and the union, `ACKNACK_Payload`) are APPENDABLE by DDS-XTypes 1.3 §7.3.1.2.1.8 and are written with a DHEADER. The `Final` setting writes the octets of the reference implementation (a `STATUS` payload of 6 octets, an `ACKNACK` of 5). The readers accept both forms in either setting. `ResultStatus::encode`, `BaseObjectReply::encode` and `TimePoint::encode_into` return or fill a `Vec<u8>`.
- `GetInfoPayload::with_client_info` and `client_info` (DDS-XRCE 1.0 §7.8.2.2, §8.4.4 Figure 10): the `client_info` (an `ObjectInfo` whose `config` is a `CLIENT_Representation`) behind the `InfoMask`. Annex A's `GET_INFO_Payload` has no member for it; a receiver that does not read it ignores the octets.
- `ObjectId::from_reference` (DDS-XRCE 1.0 §9.3, §7.7.6): the `ObjectId` of an object a configuration file defines, from the first two bytes of the MD5 of its `REPRESENTATION_BY_REFERENCE` string and the `ObjectKind`. Dependency on `md-5`.
- `REFERENCE_MAX_LEN` (128, DDS-XRCE 1.0 §7.7.3.1): a `REPRESENTATION_BY_REFERENCE` with a longer `object_reference` is refused when read and when written (`PayloadTooLarge`). It was bound only by `REPR_MAX_INLINE_BYTES`.
- `InfoMask` (DDS-XRCE 1.0 §7.7.12), `ObjectConfig`; `GetInfoPayload::new`/`parse` and `InfoPayload::new`/`parse` (`GET_INFO_Payload : BaseObjectRequest { InfoMask }`, `INFO_Payload : BaseObjectReply { ObjectInfo }`, §8.3.5.3, §8.3.5.7); `ReliableStreamState::highest_acked`.
- `ClientRepresentation`, `AgentRepresentation`, `ObjectInfo`, `ActivityInfoVariant`, `ObjectVariant` and `TransportLocator` have `encode_into` and `decode_at` (and `encode_for_kind_into`, `decode_for_kind_at`) that take the position in the payload: the padding XCDR2 puts before a `PropertySeq`, an `unsigned long` or a DHEADER depends on it (a `STATUS_AGENT` has a `ResultStatus` in front, an `INFO` a `BaseObjectReply`).
- `ClientRepresentation`, `AgentRepresentation` (with their `PropertySeq`), `ObjectInfo` and `ActivityInfoVariant` take the byte order of the submessage's E flag: `encode_with`/`decode_with`; `CreateClientPayload::into_submessage_with` and `try_from_submessage` follow the flag (DDS-XRCE 1.0 §8.3.4.2). They were always little-endian.
- `transport_dtls` (feature `dtls`): `WebrtcDtlsListener` and `WebrtcDtlsConn` (a DTLS 1.2 server with the certificate and key of PEM files, an optional client CA, a multi-thread runtime of its own, connections usable from two threads), `WebrtcDtls::connect_verified` (the client checks the server certificate against a CA file and may present its own), `load_certificate`, `load_trust_anchors`, `DtlsError::Config`.
- `transport_tls` (feature `tls`): `RustlsTlsServer::bind_with_certificate`, `try_accept`, `set_nonblocking`, `RustlsTlsClient::connect_verified`, `RustlsTlsStream::set_read_timeout` and `poll_message` (frames kept across a read timeout), `TlsConfigError`.

### Fixed
- `ObjectStore::create` with `reuse` and `replace` replaces an object whose representation does not match (DDS-XRCE 1.0 §7.8.3.1 Table 5: "delete existing object [...] and then create a new object", `STATUS_OK`); it answered `Reused { equal: false }` (`STATUS_ERR_MISMATCH`).
- `ReadSpecification` (DDS-XRCE 1.0 §7.7.14, Annex A): `data_format` carries the values 0x00, 0x02, 0x08, 0x0A and 0x0E (`DataFormat::raw`/`from_raw`, `FORMAT_MASK` 0x0E; they were 0, 1, 4, 5, 7, so FORMAT_DATA_SEQ read as FORMAT_DATA and FORMAT_SAMPLE as an error); `content_filter_expression` is an XCDR2 string (length with the NUL, aligned to 4, NUL); `DataDeliveryControl` is written with the DHEADER of an APPENDABLE struct. A `DataDeliveryControl` without DHEADER (what the eProsima client sends, and the capture holds) and one with bit 31 set in the DHEADER (Annex B.4.2) are still read; `DeliveryControlForm::Bare` and `ReadDataPayload::into_submessage_with` write the form without.
- `ObjectVariant` reads the `REPRESENTATION_IN_BINARY` of the Annex B.2 examples: a length with bit 31 set is the DHEADER of the structure (`0x80000002`, `0x8000000C`) with the members it counts, which XCDR2 would give a `sequence<octet>` length of its own.
- `ActivityInfoVariant` and `ObjectInfo` follow Annex A (§7.7.12, §7.7.13): `AGENT_ActivityInfo { short availability; TransportLocatorSeq address_seq; }`, `DATAREADER_ActivityInfo { short highest_acked_num; }` and `DATAWRITER_ActivityInfo { unsigned long long sample_seq_num; short stream_seq_num; }` with the DHEADER of an APPENDABLE type; `ObjectInfo` writes `activity` before `config`, each `@optional` as a presence octet, `config` as an `ObjectVariant` with its `ObjectKind` octet (the agent's as `AGENT_Representation`). Fields changed: `ActivityInfoVariant::Agent.availability` is `i16` and `address_seq` a `Vec<TransportLocator>`; `DataReader` has only `highest_acked_num`; `DataWriter` has `sample_seq_num: u64` and `stream_seq_num: i16`; `ObjectInfo.config` is `Option<ObjectConfig>`; `encode` of both return `Result`.
- The `PropertySeq` of `ClientRepresentation`/`AgentRepresentation` is aligned as XCDR2 does (Annex A): the element count is an `unsigned long` at the next multiple of 4, each string length as well; it was written right behind the presence octet.
- `TransportLocator` follows Annex A and §11.2.1, §11.3.1: `TransportLocatorSmall` is `address[2]` and a one-octet port, `TransportLocatorMedium` an `unsigned short` port and `TransportLocatorLarge` an `unsigned long` port, both as numbers (`u16`, `u32`) in the byte order of the E flag, after the padding XCDR2 puts before them; the length of a string locator is aligned to 4 and no longer capped at 256 (`TRANSPORT_LOCATOR_STRING_MAX` is now 65 535). The variants `Tcp` and `Serial` (discriminators 0x04 and 0x05, which `TransportLocatorFormat` does not have), `TransportLocatorTcp`, `TransportLocatorSerial` and `TRANSPORT_LOCATOR_SERIAL_DEVICE_MAX` are removed. New: `TransportLocator::encode_into`/`decode_at` (padding counted from the start of the payload), `encode_locator_seq`/`decode_locator_seq` for `TransportLocatorSeq`.
- A message without a submessage is refused: `Message::new`, `Message::encode` and `Message::decode` return `ValueOutOfRange` (DDS-XRCE 1.0 §8.3.1: "one or more Submessages"); it was accepted and round-tripped.
- Added `SessionId::is_none` and `SessionId::none_like` for the `SESSION_ID_NONE_*` values (§8.3.2.1).
- `ClientRepresentation::decode` and `AgentRepresentation::decode` return an error for a present `PropertySeq` that does not parse (DDS-XRCE 1.0 §7.8.2.1: the properties are checked); it used to read as no properties.
- `OBJECTID_INVALID` changed from `0xFFFF` to `0x0000` per DDS-XRCE 1.0 §7.6; added `OBJECTID_SESSION = 0xFFFF` (Annex A).
- `OBJK_DOMAIN` renamed to `OBJK_OTHER` per DDS-XRCE 1.0 Annex A.
- The XML snippet an `xml_config` type or QoS profile is stored as
  (`ObjectVariant::ByXmlString`) carried the indentation between child
  elements as text, since `zerodds_xml` keeps the whitespace of an element
  as part of its value (DDS-XML 1.0 §7.1.4 Tab.7.1). Whitespace between
  child elements is left out again.

## [1.0.0-rc.1] — 2026-05-07

Initial release materialization of the **`zerodds-xrce`** crate as a Layer-7 profile.

### Spec-Referenzen
Siehe `README.md` + `docs/spec-coverage/<spec>.md`.

### Public-API
Siehe `README.md` + `src/lib.rs` Doc-Comments.

### Implementation
DDS-XRCE Wire-Codec (16 Submessages, MessageHeader, RFC-1982, UDP-Mapping)

### Architektur
- Layer: 7 (Profiles)

### Stabilitaet
All `pub` items are RC1-stable; breaking changes require a major bump.
