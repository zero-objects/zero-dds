# OPC UA Part 14 — PubSub (UADP) — Spec Coverage

**Source:** `internal/standards/cache/opcfoundation/opcua-part14-pubsub-1.05.06.pdf`
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

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::uapubsub_6_2_3_field_metadata_scalar_defaults`,
`crates/opcua-pubsub/src/uadp/discovery.rs::tests::uapubsub_6_2_3_2_3_data_set_metadata_is_its_schema_header_then_its_fields`.

**Status:** done

### §6.2.4 DataSetWriter + DataSetFieldContentMask

**Spec:** §6.2.4, p. 40 + Table 32/33, p. 41 (PDF) — DataSetWriter parameters;
`DataSetFieldContentMask` selects the field encoding (Variant/RawData/DataValue
with status/timestamps).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`DataSetWriterConfig`, `DataSetFieldContentMask`),
`crates/opcua-pubsub/src/writer.rs` (`DataSetWriter`).

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::uapubsub_6_2_4_content_mask_selects_field_encoding`,
`crates/opcua-pubsub/src/writer.rs::tests::uapubsub_6_2_4_data_value_mask_projects_selected_members`.

**Status:** done

### §6.2.6 WriterGroup

**Spec:** §6.2.6, p. 47 (PDF) — WriterGroup parameters (WriterGroupId,
PublishingInterval, KeepAliveTime, Priority, MessageSettings).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`WriterGroupConfig`, `NetworkMessageContentMask`),
`crates/opcua-pubsub/src/writer.rs` (`WriterGroup`).

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::uapubsub_6_2_6_writer_group_default_mask_has_payload_and_group_header`,
`crates/opcua-pubsub/src/writer.rs::tests::uapubsub_6_2_6_writer_group_frames_with_group_header_and_publisher`.

**Status:** done

### §6.2.7 PubSubConnection

**Spec:** §6.2.7, p. 50 (PDF) — PubSubConnection parameters (PublisherId,
TransportProfileUri, Address).

**Repo:** `crates/opcua-pubsub/src/config.rs` (`PubSubConnectionConfig`).

**Tests:** cross-ref `infomodel::tests::uapubsub_6_2_7_builds_and_browses_hierarchy`.

**Status:** done

### §6.2.8 / §6.2.9 ReaderGroup + DataSetReader

**Spec:** §6.2.8, p. 53 + §6.2.9, p. 54 (PDF) — ReaderGroup and DataSetReader
parameters (Publisher/WriterGroup/DataSetWriter filters).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`ReaderGroupConfig`, `DataSetReaderConfig`),
`crates/opcua-pubsub/src/reader.rs` (`DataSetReader`, `ReaderGroup`).

**Tests:** `crates/opcua-pubsub/src/reader.rs::tests::uapubsub_6_2_8_publisher_and_writer_filters`,
`crates/opcua-pubsub/src/reader.rs::tests::uapubsub_6_2_8_reader_group_dispatches_to_matching_readers`.

**Status:** done

### §5.2.3 DataSetMetaData + custom DataType descriptions

**Spec:** §5.2.3, p. 8 + Table 7/8 (FieldMetaData), p. 30-31 (PDF); custom
DataTypes via Part 3 §8 (StructureDefinition/EnumDefinition).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`FieldMetaData`, `StructureDefinition`, `StructureField`, `EnumDefinition`,
`SimpleTypeDescription`), wire codec in
`crates/opcua-pubsub/src/uadp/discovery.rs`.

**Tests:** `crates/opcua-pubsub/src/uadp/discovery.rs::tests::uapubsub_6_2_3_2_3_the_schema_header_carries_structure_enum_and_simple_types`,
`crates/opcua-pubsub/src/uadp/discovery.rs::tests::uapubsub_6_2_3_2_4_field_metadata_carries_flags_rank_dimensions_and_properties`.

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

**Tests:** `crates/opcua-pubsub/src/uadp/network_message.rs::tests::uapubsub_7_2_4_4_every_header_field_in_the_order_of_table_154`,
`uapubsub_7_2_4_4_the_publisher_id_types_of_extended_flags1`,
`uapubsub_7_2_4_bad_version_rejected_on_decode`.

**Status:** done

### §7.2.4 DataSetMessage

**Spec:** §7.2.4, p. 99 + Figure 34, p. 108 + Table 101/102
(UadpDataSetMessageContentMask), p. 76 (PDF) — DataSetFlags1/2, optional header
fields, frame kinds KeyFrame/DeltaFrame/Event/KeepAlive.

**Repo:** `crates/opcua-pubsub/src/uadp/dataset_message.rs`
(`DataSetMessage`, `DataSetMessageKind`).

**Tests:** `crates/opcua-pubsub/src/uadp/dataset_message.rs::tests::uapubsub_7_2_4_5_4_every_header_field_in_the_order_of_table_162`,
`uapubsub_7_2_4_5_6_a_delta_frame_is_field_index_and_value_per_field`, `uapubsub_7_2_4_5_8_a_keep_alive_is_the_header_alone`.

**Status:** done

### §7.2.4 Field encodings (Variant / DataValue / RawData)

**Spec:** §7.2.4, p. 99 + Table 34 (UADP DataSetMessage field representation),
p. 42 (PDF) — Variant (self-describing), DataValue, or RawData (no type tags,
needs DataSetMetaData).

**Repo:** `crates/opcua-pubsub/src/uadp/dataset_message.rs` (`FieldEncoding`,
`DataSetData`), RawData encode in `crates/opcua-pubsub/src/writer.rs` (`encode_raw_fields`), typed
RawData decode in `crates/opcua-pubsub/src/dynamic.rs` (`decode_raw_dataset` —
struct/union/optional/enum/simple/array, recursive).

**Tests:** `crates/opcua-pubsub/src/reader.rs::tests::uapubsub_7_2_4_5_11_raw_data_fields_are_the_values_of_their_metadata_types`,
`crates/opcua-pubsub/src/dynamic.rs::tests::uapubsub_7_2_4_decodes_plain_custom_struct`,
`uapubsub_7_2_4_decodes_optional_fields_via_mask`, `uapubsub_7_2_4_decodes_union_switch`,
`uapubsub_7_2_4_decodes_nested_struct_and_array`.

**Status:** done

### §7.2.4 Payload size array + PromotedFields

**Spec:** §7.2.4, p. 99 + Figure 33, p. 107 (PDF) — the size array delimits
multiple DataSetMessages; PromotedFields for subscriber filtering without
decoding the payload.

**Repo:** `crates/opcua-pubsub/src/uadp/network_message.rs`
(`encode_payload`, `decode_payload`, PromotedFields in the header).

**Tests:** `crates/opcua-pubsub/src/uadp/network_message.rs::tests::uapubsub_7_2_4_4_the_network_messages_open62541_writes`,
`uapubsub_7_2_4_raw_data_message_bounded_by_size_array`,
`uapubsub_7_2_4_multiple_messages_without_payload_header_rejected`.

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

**Tests:** `crates/opcua-pubsub/src/uadp/discovery.rs::tests::uapubsub_7_2_4_6_12_a_publisher_information_probe_is_the_octets_of_tables_178_to_180`,
`uapubsub_7_2_4_6_4_a_metadata_announcement_is_the_octets_of_table_170`,
`uapubsub_7_2_4_6_6_an_endpoints_announcement_is_the_octets_of_table_172`,
`uapubsub_7_2_4_6_9_a_writer_configuration_announcement_is_the_octets_of_table_175`.

**Status:** done

---

## §7.2.5 JSON Message Mapping

### §7.2.5 JSON NetworkMessage + DataSetMessage (`ua-data`)

**Spec:** §7.2.5, p. 121 (PDF) — JSON NetworkMessage (MessageId/MessageType/
PublisherId/DataSetClassId/Messages) + JSON DataSetMessage
(ua-keyframe/deltaframe/keepalive/event, named payload).

**Repo:** `crates/opcua-pubsub/src/json.rs`
(`JsonNetworkMessage`, `JsonDataSetMessage`, feature `json`).

**Tests:** `crates/opcua-pubsub/src/json.rs::tests::uapubsub_7_2_5_a_network_message_carries_the_fields_of_7_2_5_3_and_7_2_5_4`,
`uapubsub_7_2_5_rejects_non_ua_data`, `uacs_5_4_2_18_a_data_value_field_is_value_status_and_timestamps`.

**Status:** done

### §7.2.5 + Part 6 §5.4 — reversible JSON value encoding

**Spec:** §7.2.5, p. 121 (PDF) + Part 6 §5.4 (`opcua-part6-mappings-1.05.07.pdf`)
— reversible JSON encoding (Int64-as-string, NaN/Inf-as-string,
ByteString-base64, Guid string, DateTime ISO 8601, NodeId/QualifiedName/
LocalizedText/ExtensionObject objects).

**Repo:** `crates/opcua-pubsub/src/json.rs`
(`variant_to_json`, `scalar_to_json`, `ticks_to_iso8601`, `guid_to_string`).

**Tests:** `crates/opcua-pubsub/src/json.rs::tests::uacs_h_8_a_reversible_variant_is_type_and_body`,
`uacs_5_4_2_4_special_floats_are_the_strings_nan_infinity_and_minus_infinity`, `uacs_5_4_2_6_a_datetime_is_the_iso_8601_string_of_its_ticks`,
`uacs_5_1_3_a_guid_is_the_string_of_the_example`.

**Status:** done

---

## §7.3 Transport Protocol Mappings

### §7.3.2 OPC UA UDP

**Spec:** §7.3.2, p. 133 (PDF) — UADP over UDP unicast/multicast, default port
4840.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`UdpTransport`,
`PubSubTransport`, `DEFAULT_UADP_PORT`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::uapubsub_7_3_2_a_network_message_is_one_udp_datagram`.

**Status:** done

### §7.3.4 MQTT

**Spec:** §7.3.4, p. 139 (PDF) — UADP/JSON over MQTT with a broker topic.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`MqttTransport`,
`MqttClient`, `mqtt_topic`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::uapubsub_7_3_4_9_a_network_message_is_one_mqtt_message_on_the_configured_topic`,
`uapubsub_7_3_4_mqtt_topic_convention`.

**Status:** done

### §B.3 AMQP (Annex B, informative)

**Spec:** Annex B.3, p. 267-271 (PDF, informative) — UADP/JSON over AMQP.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`AmqpTransport`,
`AmqpClient` — signature matching `zerodds-amqp-endpoint`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::amqp_transport_sends_one_message_to_the_configured_address`.

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

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::uapubsub_7_3_3_a_network_message_is_the_payload_of_one_frame`.

**Status:** done

### §B.2 Kafka (Annex B, informative)

**Spec:** Annex B.2, p. 266-267 (PDF, informative) — UADP/JSON over Apache
Kafka.

**Repo:** `crates/opcua-pubsub/src/transport.rs`
(`KafkaTransport`, `KafkaClient`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::kafka_transport_produces_one_record_on_the_configured_topic`.

**Status:** done

---

## §8 PubSub Security

### §7.2.4 SecurityHeader

**Spec:** §7.2.4, p. 99 + Annex A.2 (Figure A.2/A.3 — signed/encrypted layout),
p. 248 (PDF) — SecurityFlags, SecurityTokenId, NonceLength, MessageNonce,
SecurityFooterSize.

**Repo:** `crates/opcua-pubsub/src/security.rs` (`SecurityHeader`,
feature `security`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::uapubsub_7_2_4_4_3_a_signed_network_message_is_header_payload_and_hmac`.

**Status:** done

### §5.3.5 SecurityPolicies — CTR

**Spec:** §5.3.5 Message security, p. 11 (PDF) + SecurityPolicy algorithms in
Part 7 (`opcua-part7-profiles-1.05.02.pdf`) — `PubSub-Aes128-CTR` /
`PubSub-Aes256-CTR`: AES-CTR + HMAC-SHA256.

**Repo:** `crates/opcua-pubsub/src/security.rs`
(`SecurityPolicy::Aes128Ctr|Aes256Ctr`, `protect`, `unprotect`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::uapubsub_7_2_4_4_3_2_an_encrypted_network_message_is_aes_ctr_of_the_payload_then_the_hmac`, `uapubsub_5_3_5_tampered_payload_is_rejected`.

**Status:** done

### §5.3.5 SecurityPolicies — GCM

**Spec:** §5.3.5, p. 11 (PDF) + Part 7 — `PubSub-Aes256-GCM`: AES-256-GCM (AEAD).

**Repo:** `crates/opcua-pubsub/src/security.rs`
(`SecurityPolicy::Aes256Gcm`, `is_aead`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::aes256_gcm_policy_seals_the_payload_with_the_header_as_aad`,
`uapubsub_5_3_5_aead_gcm_tag_detects_tampering`.

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

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::uapubsub_8_3_2_get_security_keys_for_method_result`,
`uapubsub_8_3_2_sks_rotation_and_lookup`, `uapubsub_8_3_2_get_security_keys_snapshot`.

**Status:** done

---

## §9 PubSub Configuration Model (Information Model)

### §9.1.3 Types for the PublishSubscribe object

**Spec:** §9.1.3, p. 169 (PDF) — the PublishSubscribe object + PubSubConnection/
WriterGroup/DataSetWriter/ReaderGroup/DataSetReader/PublishedDataSet types as a
browsable AddressSpace.

**Repo:** `crates/opcua-pubsub/src/infomodel.rs`
(`PubSubConfiguration`, `nodes`, NodeId constants).

**Tests:** `crates/opcua-pubsub/src/infomodel.rs::tests::uapubsub_6_2_7_builds_and_browses_hierarchy`.

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

**Tests:** `crates/opcua-pubsub/src/infomodel.rs::tests::uapubsub_9_1_5_add_to_unknown_group_is_rejected`,
`uapubsub_9_1_5_remove_prunes_subtree_entries`.

**Status:** done

---

## Cross-spec: Part 6 / Part 4 / Part 3

### Part 6 §5.2 — OPC-UA binary encoding

**Spec:** Part 6 §5.2 (`opcua-part6-mappings-1.05.07.pdf`) — little-endian, no
alignment padding; String/ByteString/array with Int32 length prefix;
NodeId/ExpandedNodeId/Guid/QualifiedName/LocalizedText/Variant/DataValue/
ExtensionObject.

**Repo:** `crates/opcua-pubsub/src/binary/` (`crates/opcua-pubsub/src/binary/io.rs`, `crates/opcua-pubsub/src/binary/builtin.rs`, `crates/opcua-pubsub/src/binary/mod.rs`).

**Tests:** `crates/opcua-pubsub/src/binary/io.rs::tests::uacs_5_2_2_2_the_integer_of_figure_2_is_little_endian`,
`crates/opcua-pubsub/src/binary/builtin.rs::tests::uacs_5_2_2_16_an_array_variant_has_its_length_and_its_dimensions_after_the_values`,
`uacs_5_2_2_9_the_four_nodeid_forms_of_tables_17_to_20`, `uacs_5_2_2_13_a_qualified_name_is_namespace_index_and_name`.

**Status:** done

### Part 4 §7.10 — EndpointDescription

**Spec:** Part 4 §7.10 (`opcua-part4-services-1.05.07.pdf`) —
EndpointDescription (+ ApplicationDescription, UserTokenPolicy,
MessageSecurityMode).

**Repo:** `crates/opcua-pubsub/src/uadp/datatypes.rs`.

**Tests:** `crates/opcua-pubsub/src/uadp/datatypes.rs::tests::uacs_7_14_an_endpoint_description_is_its_fields_in_the_order_of_7_14`,
`uapubsub_7_10_security_mode_discriminant_rejected`.

**Status:** done

### Part 3 §8 — StructureDefinition / EnumDefinition

**Spec:** Part 3 §8.48-8.51 (`opcua-part3-addressspace-1.05.06.pdf`) —
StructureDefinition/StructureField/EnumDefinition/EnumField for custom
DataTypes.

**Repo:** `crates/opcua-pubsub/src/config.rs` (model),
`crates/opcua-pubsub/src/uadp/discovery.rs` (wire codec).

**Tests:** `crates/opcua-pubsub/src/uadp/discovery.rs::tests::uapubsub_6_2_3_2_3_the_schema_header_carries_structure_enum_and_simple_types`.

**Status:** done

---

## DataSet ↔ DDS-topic bridge + daemon (ZeroDDS extension)

### §5.4.1/§5.4.2 Daemon runtime (Publisher/Subscriber)

**Spec:** §5.4.1 Publisher, p. 12 + §5.4.2 Subscriber, p. 14 (PDF) — publish
cycle / receive.

**Repo:** `crates/opcua-pubsub/src/daemon.rs` (`Publisher`, `Subscriber`,
`publish_cycle`, `poll`, secured variants).

**Tests:** `crates/opcua-pubsub/src/daemon.rs::tests::uapubsub_5_4_1_end_to_end_publish_then_poll`,
`uapubsub_5_4_1_end_to_end_secured`, `uapubsub_5_4_1_unknown_dataset_is_reported`.

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

29 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document opcua-pubsub-1.05.md`):

* `cargo nextest run --profile merge -p zerodds-opcua-pubsub --all-targets` (job `merge-rust-opcua-pubsub`) — 47 tests green.
* `cargo nextest run --profile merge -p zerodds-opcua-pubsub --all-targets --features json,security` (job `merge-rust-opcua-pubsub-features-json-security`) — 16 tests green.

63 cited tests green, 0 not green.

Note: the `Call`-service binding points (GetSecurityKeys,
AddConnection/…/RemoveX) are implemented; the OPC-UA server wire stack
(SecureChannel/Session/Read/Write/Call) that can host them remotely exists as
`zerodds-opcua-uacp` + `zerodds-opcua-server` (coverage
`opcua-client-server-1.05.en.md`).

Open points: none. Decision records: none.
