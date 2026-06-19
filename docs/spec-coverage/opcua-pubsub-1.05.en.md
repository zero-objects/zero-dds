# OPC UA Part 14 — PubSub (UADP) — Spec Coverage

**Source:** `docs/standards/cache/opcfoundation/opcua-part14-pubsub-1.05.06.pdf`
(274 pp., v1.05.06, 2025-10-31). Cross-spec:
`opcua-part6-mappings-1.05.07.pdf`, `opcua-part4-services-1.05.07.pdf`,
`opcua-part3-addressspace-1.05.06.pdf`, `opcua-part7-profiles-1.05.02.pdf`
(same cache; not tracked in the repo for IP/copyright reasons).

**Context:** A native pure-Rust `no_std + alloc` UADP stack in the crate
`zerodds-opcua-pubsub` — Part 6 binary codec, UADP NetworkMessage/
DataSetMessage framing, JSON mapping, PubSub config + discovery, security
(SecurityHeader/SKS/AES-CTR+HMAC/AES-GCM), transport carriers (UDP/MQTT/AMQP),
the Information Model and a DataSet↔DDS-topic bridge. `forbid(unsafe_code)`.

Implementation:

- `crates/opcua-pubsub/` — UADP stack, 12 modules; 90 tests (default) /
  110 tests (`--features "json security"`) green.

---

## §6.2 Configuration parameters

### §6.2.3 PublishedDataSet

**Spec:** §6.2.3, p. 28 (PDF) — PublishedDataSet parameters incl.
DataSetMetaData (name, field descriptions, ConfigurationVersion).

**Repo:** `crates/opcua-pubsub/src/writer.rs` (`PublishedDataSet`),
`crates/opcua-pubsub/src/config.rs` (`DataSetMetaData`, `FieldMetaData`,
`ConfigurationVersion`).

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::field_metadata_scalar_defaults`,
`crates/opcua-pubsub/src/uadp/discovery.rs::tests::data_set_metadata_round_trip`.

**Status:** done

### §6.2.4 DataSetWriter + DataSetFieldContentMask

**Spec:** §6.2.4, p. 40 + Table 32/33, p. 41 (PDF) — DataSetWriter parameters;
`DataSetFieldContentMask` selects the field encoding (Variant/RawData/DataValue
with status/timestamps).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`DataSetWriterConfig`, `DataSetFieldContentMask`),
`crates/opcua-pubsub/src/writer.rs` (`DataSetWriter`).

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::content_mask_selects_field_encoding`,
`crates/opcua-pubsub/src/writer.rs::tests::data_value_mask_projects_selected_members`.

**Status:** done

### §6.2.6 WriterGroup

**Spec:** §6.2.6, p. 47 (PDF) — WriterGroup parameters (WriterGroupId,
PublishingInterval, KeepAliveTime, Priority, MessageSettings).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`WriterGroupConfig`, `NetworkMessageContentMask`),
`crates/opcua-pubsub/src/writer.rs` (`WriterGroup`).

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::writer_group_default_mask_has_payload_and_group_header`,
`crates/opcua-pubsub/src/writer.rs::tests::writer_group_frames_with_group_header_and_publisher`.

**Status:** done

### §6.2.7 PubSubConnection

**Spec:** §6.2.7, p. 50 (PDF) — PubSubConnection parameters (PublisherId,
TransportProfileUri, Address).

**Repo:** `crates/opcua-pubsub/src/config.rs` (`PubSubConnectionConfig`).

**Tests:** cross-ref `infomodel::tests::builds_and_browses_hierarchy`.

**Status:** done

### §6.2.8 / §6.2.9 ReaderGroup + DataSetReader

**Spec:** §6.2.8, p. 53 + §6.2.9, p. 54 (PDF) — ReaderGroup and DataSetReader
parameters (Publisher/WriterGroup/DataSetWriter filters).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`ReaderGroupConfig`, `DataSetReaderConfig`),
`crates/opcua-pubsub/src/reader.rs` (`DataSetReader`, `ReaderGroup`).

**Tests:** `crates/opcua-pubsub/src/reader.rs::tests::publisher_and_writer_filters`,
`crates/opcua-pubsub/src/reader.rs::tests::reader_group_dispatches_to_matching_readers`.

**Status:** done

### §5.2.3 DataSetMetaData + custom DataType descriptions

**Spec:** §5.2.3, p. 8 + Table 7/8 (FieldMetaData), p. 30-31 (PDF); custom
DataTypes via Part 3 §8 (StructureDefinition/EnumDefinition).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`FieldMetaData`, `StructureDefinition`, `StructureField`, `EnumDefinition`,
`SimpleTypeDescription`), wire codec in
`crates/opcua-pubsub/src/uadp/discovery.rs`.

**Tests:** `crates/opcua-pubsub/src/uadp/discovery.rs::tests::metadata_with_custom_datatypes_round_trips`,
`field_metadata_with_properties_round_trips`.

**Status:** done

---

## §7.2.4 UADP Message Mapping

### §7.2.4 NetworkMessage header

**Spec:** §7.2.4, p. 99 + Figure 32, p. 101 + Annex A.2, p. 245 (PDF) —
UADPFlags/ExtendedFlags1/ExtendedFlags2, PublisherId (Byte/UInt16/UInt32/
UInt64/String), DataSetClassId, GroupHeader, PayloadHeader, Timestamp,
PicoSeconds, PromotedFields.

**Repo:** `crates/opcua-pubsub/src/uadp/network_message.rs`
(`NetworkMessage`, `GroupHeader`, `PublisherId`, `encode_header`,
`decode_header`).

**Tests:** `crates/opcua-pubsub/src/uadp/network_message.rs::tests::full_header_roundtrip`,
`publisher_id_all_types_roundtrip`, `byte_publisher_id_needs_no_extended_flags`,
`bad_version_rejected_on_decode`.

**Status:** done

### §7.2.4 DataSetMessage

**Spec:** §7.2.4, p. 99 + Figure 34, p. 108 + Table 101/102
(UadpDataSetMessageContentMask), p. 76 (PDF) — DataSetFlags1/2, optional header
fields, frame kinds KeyFrame/DeltaFrame/Event/KeepAlive.

**Repo:** `crates/opcua-pubsub/src/uadp/dataset_message.rs`
(`DataSetMessage`, `DataSetMessageKind`).

**Tests:** `crates/opcua-pubsub/src/uadp/dataset_message.rs::tests::key_frame_with_header_fields`,
`delta_frame_variant_roundtrip`, `keep_alive_has_no_data`.

**Status:** done

### §7.2.4 Field encodings (Variant / DataValue / RawData)

**Spec:** §7.2.4, p. 99 + Table 34 (UADP DataSetMessage field representation),
p. 42 (PDF) — Variant (self-describing), DataValue, or RawData (no type tags,
needs DataSetMetaData).

**Repo:** `crates/opcua-pubsub/src/uadp/dataset_message.rs` (`FieldEncoding`,
`DataSetData`), RawData encode in `writer.rs` (`encode_raw_fields`), typed
RawData decode in `crates/opcua-pubsub/src/dynamic.rs` (`decode_raw_dataset` —
struct/union/optional/enum/simple/array, recursive).

**Tests:** `crates/opcua-pubsub/src/reader.rs::tests::raw_data_round_trip_uses_metadata_types`,
`crates/opcua-pubsub/src/dynamic.rs::tests::decodes_plain_custom_struct`,
`decodes_optional_fields_via_mask`, `decodes_union_switch`,
`decodes_nested_struct_and_array`.

**Status:** done

### §7.2.4 Payload size array + PromotedFields

**Spec:** §7.2.4, p. 99 + Figure 33, p. 107 (PDF) — the size array delimits
multiple DataSetMessages; PromotedFields for subscriber filtering without
decoding the payload.

**Repo:** `crates/opcua-pubsub/src/uadp/network_message.rs`
(`encode_payload`, `decode_payload`, PromotedFields in the header).

**Tests:** `crates/opcua-pubsub/src/uadp/network_message.rs::tests::multiple_messages_use_size_array`,
`raw_data_message_bounded_by_size_array`,
`multiple_messages_without_payload_header_rejected`.

**Status:** done

### §7.2.4 Discovery (request/response NetworkMessages)

**Spec:** §7.2.4, p. 99-120 (PDF) — UADP discovery NetworkMessages:
DiscoveryRequest (InformationType + DataSetWriterIds) and DiscoveryResponse
(DataSetMetaData / PublisherEndpoints[EndpointDescription, Part 4 §7.10] /
DataSetWriterConfiguration[WriterGroupDataType]).

**Repo:** `crates/opcua-pubsub/src/uadp/discovery.rs`
(`DiscoveryRequest`, `DiscoveryResponse`, `InformationType`,
`DataSetMetaDataResponse`, `PublisherEndpointsResponse`,
`DataSetWriterConfigurationResponse`),
`crates/opcua-pubsub/src/uadp/datatypes.rs`
(`EndpointDescription`, `WriterGroupDataType`).

**Tests:** `crates/opcua-pubsub/src/uadp/discovery.rs::tests::discovery_request_round_trip`,
`discovery_response_metadata_round_trip`,
`discovery_response_publisher_endpoints_round_trip`,
`discovery_response_writer_configuration_round_trip`.

**Status:** done

---

## §7.2.5 JSON Message Mapping

### §7.2.5 JSON NetworkMessage + DataSetMessage (`ua-data`)

**Spec:** §7.2.5, p. 121 (PDF) — JSON NetworkMessage (MessageId/MessageType/
PublisherId/DataSetClassId/Messages) + JSON DataSetMessage
(ua-keyframe/deltaframe/keepalive/event, named payload).

**Repo:** `crates/opcua-pubsub/src/json.rs`
(`JsonNetworkMessage`, `JsonDataSetMessage`, feature `json`).

**Tests:** `crates/opcua-pubsub/src/json.rs::tests::network_message_round_trips`,
`rejects_non_ua_data`, `datavalue_payload_with_status_round_trips`.

**Status:** done

### §7.2.5 + Part 6 §5.4 — reversible JSON value encoding

**Spec:** §7.2.5, p. 121 (PDF) + Part 6 §5.4 (`opcua-part6-mappings-1.05.07.pdf`)
— reversible JSON encoding (Int64-as-string, NaN/Inf-as-string,
ByteString-base64, Guid string, DateTime ISO 8601, NodeId/QualifiedName/
LocalizedText/ExtensionObject objects).

**Repo:** `crates/opcua-pubsub/src/json.rs`
(`variant_to_json`, `scalar_to_json`, `ticks_to_iso8601`, `guid_to_string`).

**Tests:** `crates/opcua-pubsub/src/json.rs::tests::variant_reversible_round_trips_many_types`,
`special_floats_round_trip`, `datetime_round_trips_through_iso8601`,
`guid_round_trips`.

**Status:** done

---

## §7.3 Transport Protocol Mappings

### §7.3.2 OPC UA UDP

**Spec:** §7.3.2, p. 133 (PDF) — UADP over UDP unicast/multicast, default port
4840.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`UdpTransport`,
`PubSubTransport`, `DEFAULT_UADP_PORT`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::udp_unicast_localhost_round_trip`.

**Status:** done

### §7.3.4 MQTT

**Spec:** §7.3.4, p. 139 (PDF) — UADP/JSON over MQTT with a broker topic.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`MqttTransport`,
`MqttClient`, `mqtt_topic`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::mqtt_transport_round_trip`,
`mqtt_topic_convention`.

**Status:** done

### §B.3 AMQP (Annex B, informative)

**Spec:** Annex B.3, p. 267-271 (PDF, informative) — UADP/JSON over AMQP.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`AmqpTransport`,
`AmqpClient` — signature matching `zerodds-amqp-endpoint`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::amqp_transport_round_trip`.

**Status:** done

### §7.3.3 OPC UA Ethernet

**Spec:** §7.3.3, p. 138 (PDF) — UADP directly in the Ethernet frame,
EtherType 0xB62C.

**Repo:** `crates/opcua-pubsub/src/transport.rs`
(`ETHERNET_ETHERTYPE`, `EthernetTransport`, `EthernetInterface`). The
privileged raw L2 socket (AF_PACKET, `CAP_NET_RAW`, `unsafe`) is injected via
the `EthernetInterface` trait (e.g. `zerodds-transport-tsn` `live`) — the crate
stays `forbid(unsafe_code)`, exactly as the MQTT/AMQP clients keep the broker
out.

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::ethernet_transport_round_trip`.

**Status:** done

### §B.2 Kafka (Annex B, informative)

**Spec:** Annex B.2, p. 266-267 (PDF, informative) — UADP/JSON over Apache
Kafka.

**Repo:** `crates/opcua-pubsub/src/transport.rs`
(`KafkaTransport`, `KafkaClient`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::kafka_transport_round_trip`.

**Status:** done

---

## §8 PubSub Security

### §7.2.4 SecurityHeader

**Spec:** §7.2.4, p. 99 + Annex A.2 (Figure A.2/A.3 — signed/encrypted layout),
p. 248 (PDF) — SecurityFlags, SecurityTokenId, NonceLength, MessageNonce,
SecurityFooterSize.

**Repo:** `crates/opcua-pubsub/src/security.rs` (`SecurityHeader`,
feature `security`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::sign_only_round_trip`.

**Status:** done

### §5.3.5 SecurityPolicies — CTR

**Spec:** §5.3.5 Message security, p. 11 (PDF) + SecurityPolicy algorithms in
Part 7 (`opcua-part7-profiles-1.05.02.pdf`) — `PubSub-Aes128-CTR` /
`PubSub-Aes256-CTR`: AES-CTR + HMAC-SHA256.

**Repo:** `crates/opcua-pubsub/src/security.rs`
(`SecurityPolicy::Aes128Ctr|Aes256Ctr`, `protect`, `unprotect`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::encrypt_and_sign_round_trip_aes256`,
`encrypt_and_sign_round_trip_aes128`, `tampered_payload_is_rejected`.

**Status:** done

### §5.3.5 SecurityPolicies — GCM

**Spec:** §5.3.5, p. 11 (PDF) + Part 7 — `PubSub-Aes256-GCM`: AES-256-GCM (AEAD).

**Repo:** `crates/opcua-pubsub/src/security.rs`
(`SecurityPolicy::Aes256Gcm`, `is_aead`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::aead_gcm_round_trip`,
`aead_gcm_tag_detects_tampering`.

**Status:** done

### §8.3.2 Security Key Service — GetSecurityKeys

**Spec:** §8.3.2 GetSecurityKeys Method, p. 150 + §8 SKS model, p. 148 (PDF) —
the SKS manages current + future keys per SecurityGroup.

**Repo:** `crates/opcua-pubsub/src/security.rs`
(`SecurityKeyService`, `SecurityKey`, `SecurityKeys`, `get_security_keys`,
`get_security_keys_for` (method-result builder: SecurityPolicyUri/FirstTokenId/
Keys/TimeToNextKey/KeyLifetime), `rotate`, `key_for_token`). The `Call`-service
binding point is implemented; the SecureChannel/Session transport carrying the
call is provided by the OPC-UA server wire stack (`zerodds-opcua-uacp` +
`zerodds-opcua-server`, coverage `opcua-client-server-1.05.en.md`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::get_security_keys_for_method_result`,
`sks_rotation_and_lookup`, `get_security_keys_snapshot`.

**Status:** done

---

## §9 PubSub Configuration Model (Information Model)

### §9.1.3 Types for the PublishSubscribe object

**Spec:** §9.1.3, p. 169 (PDF) — the PublishSubscribe object + PubSubConnection/
WriterGroup/DataSetWriter/ReaderGroup/DataSetReader/PublishedDataSet types as a
browsable AddressSpace.

**Repo:** `crates/opcua-pubsub/src/infomodel.rs`
(`PubSubConfiguration`, `nodes`, NodeId constants).

**Tests:** `crates/opcua-pubsub/src/infomodel.rs::tests::builds_and_browses_hierarchy`.

**Status:** done

### §9.1.5 / §9.1.6 Management methods (AddConnection/…/RemoveX)

**Spec:** §9.1.5 Connection model, p. 197 + §9.1.6 Group model, p. 203 (PDF) —
methods to add/remove connections/groups/writers/readers.

**Repo:** `crates/opcua-pubsub/src/infomodel.rs`
(`add_connection`, `add_writer_group`, `add_dataset_writer`,
`add_reader_group`, `add_dataset_reader`, `add_published_data_set`, `remove`).
These operations are the `Call`-service binding points (the method semantics);
the SecureChannel/Session transport carrying the call is provided by the OPC-UA
server wire stack (`zerodds-opcua-uacp` + `zerodds-opcua-server`, coverage
`opcua-client-server-1.05.en.md`).

**Tests:** `crates/opcua-pubsub/src/infomodel.rs::tests::add_to_unknown_group_is_rejected`,
`remove_prunes_subtree_entries`.

**Status:** done

---

## Cross-spec: Part 6 / Part 4 / Part 3

### Part 6 §5.2 — OPC-UA binary encoding

**Spec:** Part 6 §5.2 (`opcua-part6-mappings-1.05.07.pdf`) — little-endian, no
alignment padding; String/ByteString/array with Int32 length prefix;
NodeId/ExpandedNodeId/Guid/QualifiedName/LocalizedText/Variant/DataValue/
ExtensionObject.

**Repo:** `crates/opcua-pubsub/src/binary/` (`io.rs`, `builtin.rs`, `mod.rs`).

**Tests:** `crates/opcua-pubsub/src/binary/io.rs::tests::primitive_roundtrip_is_little_endian`,
`crates/opcua-pubsub/src/binary/builtin.rs::tests::variant_1d_array`,
`nodeid_two_byte_compact_form`, `qualified_name_roundtrip`.

**Status:** done

### Part 4 §7.10 — EndpointDescription

**Spec:** Part 4 §7.10 (`opcua-part4-services-1.05.07.pdf`) —
EndpointDescription (+ ApplicationDescription, UserTokenPolicy,
MessageSecurityMode).

**Repo:** `crates/opcua-pubsub/src/uadp/datatypes.rs`.

**Tests:** `crates/opcua-pubsub/src/uadp/datatypes.rs::tests::endpoint_description_round_trip`,
`security_mode_discriminant_rejected`.

**Status:** done

### Part 3 §8 — StructureDefinition / EnumDefinition

**Spec:** Part 3 §8.48-8.51 (`opcua-part3-addressspace-1.05.06.pdf`) —
StructureDefinition/StructureField/EnumDefinition/EnumField for custom
DataTypes.

**Repo:** `crates/opcua-pubsub/src/config.rs` (model),
`crates/opcua-pubsub/src/uadp/discovery.rs` (wire codec).

**Tests:** `crates/opcua-pubsub/src/uadp/discovery.rs::tests::metadata_with_custom_datatypes_round_trips`.

**Status:** done

---

## DataSet ↔ DDS-topic bridge + daemon (ZeroDDS extension)

### §5.4.1/§5.4.2 Daemon runtime (Publisher/Subscriber)

**Spec:** §5.4.1 Publisher, p. 12 + §5.4.2 Subscriber, p. 14 (PDF) — publish
cycle / receive.

**Repo:** `crates/opcua-pubsub/src/daemon.rs` (`Publisher`, `Subscriber`,
`publish_cycle`, `poll`, secured variants).

**Tests:** `crates/opcua-pubsub/src/daemon.rs::tests::end_to_end_publish_then_poll`,
`end_to_end_secured`, `unknown_dataset_is_reported`.

**Status:** done

### DataSet ↔ DDS-topic bridge

**Spec:** ZeroDDS extension (not a Part-14 item) — mapping OPC-UA DataSets ↔
DDS topics.

**Repo:** `crates/opcua-pubsub/src/bridge.rs`
(`DataSetTopicMapping`, `OpcUaToDdsBridge`, `DdsToOpcUaBridge`).

**Tests:** `crates/opcua-pubsub/src/bridge.rs::tests::opcua_to_dds_and_back`,
`dataset_maps_to_dds_sample_with_renames`.

**Status:** done

---

## Audit status

29 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Note: the `Call`-service binding points (GetSecurityKeys,
AddConnection/…/RemoveX) are implemented; the OPC-UA server wire stack
(SecureChannel/Session/Read/Write/Call) that can host them remotely exists as
`zerodds-opcua-uacp` + `zerodds-opcua-server` (coverage
`opcua-client-server-1.05.en.md`).

Test run: `cargo test -p zerodds-opcua-pubsub --features "json security"` —
110 tests green, 0 failed.

Open points: none. Decision records: none.
