# OPC UA Part 14 — PubSub (UADP) — Spec-Coverage

**Quelle:** `docs/standards/cache/opcfoundation/opcua-part14-pubsub-1.05.06.pdf`
(274 S., v1.05.06, 2025-10-31). Cross-Spec:
`opcua-part6-mappings-1.05.07.pdf`, `opcua-part4-services-1.05.07.pdf`,
`opcua-part3-addressspace-1.05.06.pdf`, `opcua-part7-profiles-1.05.02.pdf`
(alle im selben Cache; aus IP-/Copyright-Gründen nicht im Repo getrackt).

**Kontext:** Nativer pure-Rust `no_std + alloc` UADP-Stack im Crate
`zerodds-opcua-pubsub` — Part-6-Binär-Codec, UADP-NetworkMessage/
DataSetMessage-Rahmung, JSON-Mapping, PubSub-Config + Discovery, Security
(SecurityHeader/SKS/AES-CTR+HMAC/AES-GCM), Transport-Carrier (UDP/MQTT/AMQP),
Information-Model und eine DataSet↔DDS-Topic-Bridge. `forbid(unsafe_code)`.

Implementation:

- `crates/opcua-pubsub/` — UADP-Stack, 12 Module; 90 Tests (default) /
  110 Tests (`--features "json security"`) grün.

---

## §6.2 Konfigurationsparameter

### §6.2.3 PublishedDataSet

**Spec:** §6.2.3, S. 28 (PDF) — PublishedDataSet-Parameter inkl.
DataSetMetaData (Name, Feldbeschreibungen, ConfigurationVersion).

**Repo:** `crates/opcua-pubsub/src/writer.rs` (`PublishedDataSet`),
`crates/opcua-pubsub/src/config.rs` (`DataSetMetaData`, `FieldMetaData`,
`ConfigurationVersion`).

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::field_metadata_scalar_defaults`,
`crates/opcua-pubsub/src/uadp/discovery.rs::tests::data_set_metadata_round_trip`.

**Status:** done

### §6.2.4 DataSetWriter + DataSetFieldContentMask

**Spec:** §6.2.4, S. 40 + Table 32/33, S. 41 (PDF) — DataSetWriter-Parameter;
`DataSetFieldContentMask` wählt Feld-Kodierung (Variant/RawData/DataValue mit
Status/Timestamps).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`DataSetWriterConfig`, `DataSetFieldContentMask`),
`crates/opcua-pubsub/src/writer.rs` (`DataSetWriter`).

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::content_mask_selects_field_encoding`,
`crates/opcua-pubsub/src/writer.rs::tests::data_value_mask_projects_selected_members`.

**Status:** done

### §6.2.6 WriterGroup

**Spec:** §6.2.6, S. 47 (PDF) — WriterGroup-Parameter (WriterGroupId,
PublishingInterval, KeepAliveTime, Priority, MessageSettings).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`WriterGroupConfig`, `NetworkMessageContentMask`),
`crates/opcua-pubsub/src/writer.rs` (`WriterGroup`).

**Tests:** `crates/opcua-pubsub/src/config.rs::tests::writer_group_default_mask_has_payload_and_group_header`,
`crates/opcua-pubsub/src/writer.rs::tests::writer_group_frames_with_group_header_and_publisher`.

**Status:** done

### §6.2.7 PubSubConnection

**Spec:** §6.2.7, S. 50 (PDF) — PubSubConnection-Parameter (PublisherId,
TransportProfileUri, Address).

**Repo:** `crates/opcua-pubsub/src/config.rs` (`PubSubConnectionConfig`).

**Tests:** Cross-Ref `infomodel::tests::builds_and_browses_hierarchy`.

**Status:** done

### §6.2.8 / §6.2.9 ReaderGroup + DataSetReader

**Spec:** §6.2.8, S. 53 + §6.2.9, S. 54 (PDF) — ReaderGroup- und
DataSetReader-Parameter (Publisher-/WriterGroup-/DataSetWriter-Filter).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`ReaderGroupConfig`, `DataSetReaderConfig`),
`crates/opcua-pubsub/src/reader.rs` (`DataSetReader`, `ReaderGroup`).

**Tests:** `crates/opcua-pubsub/src/reader.rs::tests::publisher_and_writer_filters`,
`crates/opcua-pubsub/src/reader.rs::tests::reader_group_dispatches_to_matching_readers`.

**Status:** done

### §5.2.3 DataSetMetaData + Custom-DataType-Beschreibungen

**Spec:** §5.2.3, S. 8 + Table 7/8 (FieldMetaData), S. 30-31 (PDF); Custom-
DataTypes via Part 3 §8 (StructureDefinition/EnumDefinition).

**Repo:** `crates/opcua-pubsub/src/config.rs`
(`FieldMetaData`, `StructureDefinition`, `StructureField`, `EnumDefinition`,
`SimpleTypeDescription`), Wire-Codec in
`crates/opcua-pubsub/src/uadp/discovery.rs`.

**Tests:** `crates/opcua-pubsub/src/uadp/discovery.rs::tests::metadata_with_custom_datatypes_round_trips`,
`field_metadata_with_properties_round_trips`.

**Status:** done

---

## §7.2.4 UADP-Message-Mapping

### §7.2.4 NetworkMessage-Header

**Spec:** §7.2.4, S. 99 + Figure 32, S. 101 + Annex A.2, S. 245 (PDF) —
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

**Spec:** §7.2.4, S. 99 + Figure 34, S. 108 + Table 101/102
(UadpDataSetMessageContentMask), S. 76 (PDF) — DataSetFlags1/2, optionale
Header-Felder, Frame-Kinds KeyFrame/DeltaFrame/Event/KeepAlive.

**Repo:** `crates/opcua-pubsub/src/uadp/dataset_message.rs`
(`DataSetMessage`, `DataSetMessageKind`).

**Tests:** `crates/opcua-pubsub/src/uadp/dataset_message.rs::tests::key_frame_with_header_fields`,
`delta_frame_variant_roundtrip`, `keep_alive_has_no_data`.

**Status:** done

### §7.2.4 Feld-Kodierungen (Variant / DataValue / RawData)

**Spec:** §7.2.4, S. 99 + Table 34 (UADP DataSetMessage field representation),
S. 42 (PDF) — Variant (selbstbeschreibend), DataValue oder RawData (ohne
Typ-Tags, braucht DataSetMetaData).

**Repo:** `crates/opcua-pubsub/src/uadp/dataset_message.rs` (`FieldEncoding`,
`DataSetData`), RawData-Encode in `writer.rs` (`encode_raw_fields`),
typisiertes RawData-Decode in `crates/opcua-pubsub/src/dynamic.rs`
(`decode_raw_dataset` — Struct/Union/Optional/Enum/Simple/Array, rekursiv).

**Tests:** `crates/opcua-pubsub/src/reader.rs::tests::raw_data_round_trip_uses_metadata_types`,
`crates/opcua-pubsub/src/dynamic.rs::tests::decodes_plain_custom_struct`,
`decodes_optional_fields_via_mask`, `decodes_union_switch`,
`decodes_nested_struct_and_array`.

**Status:** done

### §7.2.4 Payload-Size-Array + PromotedFields

**Spec:** §7.2.4, S. 99 + Figure 33, S. 107 (PDF) — das Size-Array begrenzt
mehrere DataSetMessages; PromotedFields für Subscriber-Filterung ohne
Payload-Decode.

**Repo:** `crates/opcua-pubsub/src/uadp/network_message.rs`
(`encode_payload`, `decode_payload`, PromotedFields im Header).

**Tests:** `crates/opcua-pubsub/src/uadp/network_message.rs::tests::multiple_messages_use_size_array`,
`raw_data_message_bounded_by_size_array`,
`multiple_messages_without_payload_header_rejected`.

**Status:** done

### §7.2.4 Discovery (Request/Response NetworkMessages)

**Spec:** §7.2.4, S. 99-120 (PDF) — UADP-Discovery-NetworkMessages:
DiscoveryRequest (InformationType + DataSetWriterIds) und DiscoveryResponse
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

## §7.2.5 JSON-Message-Mapping

### §7.2.5 JSON-NetworkMessage + DataSetMessage (`ua-data`)

**Spec:** §7.2.5, S. 121 (PDF) — JSON-NetworkMessage (MessageId/MessageType/
PublisherId/DataSetClassId/Messages) + JSON-DataSetMessage
(ua-keyframe/deltaframe/keepalive/event, benannter Payload).

**Repo:** `crates/opcua-pubsub/src/json.rs`
(`JsonNetworkMessage`, `JsonDataSetMessage`, Feature `json`).

**Tests:** `crates/opcua-pubsub/src/json.rs::tests::network_message_round_trips`,
`rejects_non_ua_data`, `datavalue_payload_with_status_round_trips`.

**Status:** done

### §7.2.5 + Part 6 §5.4 — reversibles JSON-Wert-Encoding

**Spec:** §7.2.5, S. 121 (PDF) + Part 6 §5.4 (`opcua-part6-mappings-1.05.07.pdf`)
— reversibles JSON-Encoding (Int64-als-String, NaN/Inf-als-String,
ByteString-base64, Guid-String, DateTime-ISO8601, NodeId/QualifiedName/
LocalizedText/ExtensionObject-Objekte).

**Repo:** `crates/opcua-pubsub/src/json.rs`
(`variant_to_json`, `scalar_to_json`, `ticks_to_iso8601`, `guid_to_string`).

**Tests:** `crates/opcua-pubsub/src/json.rs::tests::variant_reversible_round_trips_many_types`,
`special_floats_round_trip`, `datetime_round_trips_through_iso8601`,
`guid_round_trips`.

**Status:** done

---

## §7.3 Transport-Protocol-Mappings

### §7.3.2 OPC UA UDP

**Spec:** §7.3.2, S. 133 (PDF) — UADP über UDP-Unicast/Multicast, Default-Port
4840.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`UdpTransport`,
`PubSubTransport`, `DEFAULT_UADP_PORT`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::udp_unicast_localhost_round_trip`.

**Status:** done

### §7.3.4 MQTT

**Spec:** §7.3.4, S. 139 (PDF) — UADP/JSON über MQTT mit Broker-Topic.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`MqttTransport`,
`MqttClient`, `mqtt_topic`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::mqtt_transport_round_trip`,
`mqtt_topic_convention`.

**Status:** done

### §B.3 AMQP (Annex B, informativ)

**Spec:** Annex B.3, S. 267-271 (PDF, informativ) — UADP/JSON über AMQP.

**Repo:** `crates/opcua-pubsub/src/transport.rs` (`AmqpTransport`,
`AmqpClient` — Signatur passend zu `zerodds-amqp-endpoint`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::amqp_transport_round_trip`.

**Status:** done

### §7.3.3 OPC UA Ethernet

**Spec:** §7.3.3, S. 138 (PDF) — UADP direkt im Ethernet-Frame,
EtherType 0xB62C.

**Repo:** `crates/opcua-pubsub/src/transport.rs`
(`ETHERNET_ETHERTYPE`, `EthernetTransport`, `EthernetInterface`). Der
privilegierte Raw-L2-Socket (AF_PACKET, `CAP_NET_RAW`, `unsafe`) wird über das
`EthernetInterface`-Trait injiziert (z. B. `zerodds-transport-tsn` `live`) —
das Crate bleibt `forbid(unsafe_code)`, genau wie bei MQTT/AMQP-Clients.

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::ethernet_transport_round_trip`.

**Status:** done

### §B.2 Kafka (Annex B, informativ)

**Spec:** Annex B.2, S. 266-267 (PDF, informativ) — UADP/JSON über Apache
Kafka.

**Repo:** `crates/opcua-pubsub/src/transport.rs`
(`KafkaTransport`, `KafkaClient`).

**Tests:** `crates/opcua-pubsub/src/transport.rs::tests::kafka_transport_round_trip`.

**Status:** done

---

## §8 PubSub-Security

### §7.2.4 SecurityHeader

**Spec:** §7.2.4, S. 99 + Annex A.2 (Figure A.2/A.3 — signed/encrypted layout),
S. 248 (PDF) — SecurityFlags, SecurityTokenId, NonceLength, MessageNonce,
SecurityFooterSize.

**Repo:** `crates/opcua-pubsub/src/security.rs` (`SecurityHeader`,
Feature `security`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::sign_only_round_trip`.

**Status:** done

### §5.3.5 SecurityPolicies — CTR

**Spec:** §5.3.5 Message security, S. 11 (PDF) + SecurityPolicy-Algorithmen in
Part 7 (`opcua-part7-profiles-1.05.02.pdf`) — `PubSub-Aes128-CTR` /
`PubSub-Aes256-CTR`: AES-CTR + HMAC-SHA256.

**Repo:** `crates/opcua-pubsub/src/security.rs`
(`SecurityPolicy::Aes128Ctr|Aes256Ctr`, `protect`, `unprotect`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::encrypt_and_sign_round_trip_aes256`,
`encrypt_and_sign_round_trip_aes128`, `tampered_payload_is_rejected`.

**Status:** done

### §5.3.5 SecurityPolicies — GCM

**Spec:** §5.3.5, S. 11 (PDF) + Part 7 — `PubSub-Aes256-GCM`: AES-256-GCM (AEAD).

**Repo:** `crates/opcua-pubsub/src/security.rs`
(`SecurityPolicy::Aes256Gcm`, `is_aead`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::aead_gcm_round_trip`,
`aead_gcm_tag_detects_tampering`.

**Status:** done

### §8.3.2 Security Key Service — GetSecurityKeys

**Spec:** §8.3.2 GetSecurityKeys Method, S. 150 + §8 SKS-Model, S. 148 (PDF) —
SKS verwaltet aktuelle + zukünftige Schlüssel pro SecurityGroup.

**Repo:** `crates/opcua-pubsub/src/security.rs`
(`SecurityKeyService`, `SecurityKey`, `SecurityKeys`, `get_security_keys`,
`get_security_keys_for` (Method-Result-Builder: SecurityPolicyUri/FirstTokenId/
Keys/TimeToNextKey/KeyLifetime), `rotate`, `key_for_token`). Der `Call`-Service-
Binding-Point ist implementiert; die SecureChannel/Session-Übertragung des
Calls liefert der OPC-UA-Server-Wire-Stack (`zerodds-opcua-uacp` +
`zerodds-opcua-server`, Coverage `opcua-client-server-1.05.md`).

**Tests:** `crates/opcua-pubsub/src/security.rs::tests::get_security_keys_for_method_result`,
`sks_rotation_and_lookup`, `get_security_keys_snapshot`.

**Status:** done

---

## §9 PubSub-Konfigurations-Model (Information Model)

### §9.1.3 Typen für das PublishSubscribe-Objekt

**Spec:** §9.1.3, S. 169 (PDF) — PublishSubscribe-Objekt + PubSubConnectionType/
WriterGroupType/DataSetWriterType/ReaderGroupType/DataSetReaderType/
PublishedDataSetType als browsbare AddressSpace.

**Repo:** `crates/opcua-pubsub/src/infomodel.rs`
(`PubSubConfiguration`, `nodes`, NodeId-Konstanten).

**Tests:** `crates/opcua-pubsub/src/infomodel.rs::tests::builds_and_browses_hierarchy`.

**Status:** done

### §9.1.5 / §9.1.6 Management-Methoden (AddConnection/…/RemoveX)

**Spec:** §9.1.5 Connection model, S. 197 + §9.1.6 Group model, S. 203 (PDF) —
Methoden zum Hinzufügen/Entfernen von Connections/Groups/Writers/Readers.

**Repo:** `crates/opcua-pubsub/src/infomodel.rs`
(`add_connection`, `add_writer_group`, `add_dataset_writer`,
`add_reader_group`, `add_dataset_reader`, `add_published_data_set`, `remove`).
Diese Operationen sind die `Call`-Service-Binding-Points (die Method-Semantik);
die SecureChannel/Session-Übertragung des Calls liefert der OPC-UA-Server-Wire-
Stack (`zerodds-opcua-uacp` + `zerodds-opcua-server`, Coverage
`opcua-client-server-1.05.md`).

**Tests:** `crates/opcua-pubsub/src/infomodel.rs::tests::add_to_unknown_group_is_rejected`,
`remove_prunes_subtree_entries`.

**Status:** done

---

## Cross-Spec: Part 6 / Part 4 / Part 3

### Part 6 §5.2 — OPC-UA-Binär-Encoding

**Spec:** Part 6 §5.2 (`opcua-part6-mappings-1.05.07.pdf`) — Little-Endian,
kein Alignment-Padding; String/ByteString/Array mit Int32-Längenpräfix;
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
StructureDefinition/StructureField/EnumDefinition/EnumField für Custom-
DataTypes.

**Repo:** `crates/opcua-pubsub/src/config.rs` (Modell),
`crates/opcua-pubsub/src/uadp/discovery.rs` (Wire-Codec).

**Tests:** `crates/opcua-pubsub/src/uadp/discovery.rs::tests::metadata_with_custom_datatypes_round_trips`.

**Status:** done

---

## DataSet ↔ DDS-Topic-Bridge + Daemon (ZeroDDS-Erweiterung)

### §5.4.1/§5.4.2 Daemon-Laufzeit (Publisher/Subscriber)

**Spec:** §5.4.1 Publisher, S. 12 + §5.4.2 Subscriber, S. 14 (PDF) —
Publish-Zyklus / Empfang.

**Repo:** `crates/opcua-pubsub/src/daemon.rs` (`Publisher`, `Subscriber`,
`publish_cycle`, `poll`, secured-Varianten).

**Tests:** `crates/opcua-pubsub/src/daemon.rs::tests::end_to_end_publish_then_poll`,
`end_to_end_secured`, `unknown_dataset_is_reported`.

**Status:** done

### DataSet ↔ DDS-Topic-Bridge

**Spec:** ZeroDDS-Erweiterung (kein Part-14-Item) — Mapping OPC-UA-DataSets ↔
DDS-Topics.

**Repo:** `crates/opcua-pubsub/src/bridge.rs`
(`DataSetTopicMapping`, `OpcUaToDdsBridge`, `DdsToOpcUaBridge`).

**Tests:** `crates/opcua-pubsub/src/bridge.rs::tests::opcua_to_dds_and_back`,
`dataset_maps_to_dds_sample_with_renames`.

**Status:** done

---

## Audit-Status

29 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Hinweis: die `Call`-Service-Binding-Points (GetSecurityKeys,
AddConnection/…/RemoveX) sind implementiert; der OPC-UA-Server-Wire-Stack
(SecureChannel/Session/Read/Write/Call), der sie remote hosten kann, existiert
als `zerodds-opcua-uacp` + `zerodds-opcua-server` (Coverage
`opcua-client-server-1.05.md`).

Test-Lauf: `cargo test -p zerodds-opcua-pubsub --features "json security"` —
110 Tests grün, 0 failed.

Offene Punkte: keine. Decision-Records: keine.
