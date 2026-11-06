# OASIS MQTT v5.0 — Spec-Coverage

**Spec:** [OASIS MQTT Version 5.0 — Standard, 07 March 2019 →](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html)

**Kontext:** MQTT 5.0 ist die OASIS-Pub-Sub-Spec für IoT/M2M.
ZeroDDS implementiert den **Wire-Codec** (Spec §1.5 Data Types,
§2.1 Fixed Header, §2.2 Variable Header + Properties, §3.3 PUBLISH)
als pure-Rust no_std+alloc Library. Broker-Logic (Session-State,
Topic-Filter-Matching, QoS-Retransmission) ist Caller-Layer.

Implementation:

- `crates/mqtt-bridge/` — Wire-Codec, 5 Module, 36 Tests grün.

---

## §1 Introduction

### §1.1-§1.4 Background

**Spec:** §1, S. 7-10 (Spec) — Abstract, Status, Status of this Doc.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Editorial-Hintergrund; ohne Code-Mapping.

### §1.5 Data Types

**Spec:** §1.5 — alle MQTT-Datatypes:
* §1.5.1 Bits — implizit via `u8`.
* §1.5.2 Two Byte Integer (BE u16).
* §1.5.3 Four Byte Integer (BE u32) — implizit via `u32::to_be_bytes`.
* §1.5.4 UTF-8 Encoded String (u16 BE length + UTF-8).
* §1.5.5 Variable Byte Integer (1-4 bytes, 7-bit-groups +
  continuation-bit, max 268,435,455).
* §1.5.6 Binary Data (u16 BE length + bytes).
* §1.5.7 UTF-8 String Pair — Composition aus 2 §1.5.4-Strings (Caller
  baut via 2 `encode_utf8_string`-Aufrufen).

**Repo:** `crates/mqtt-bridge/src/data_types.rs` +
`crates/mqtt-bridge/src/vbi.rs`.

**Tests:** `data_types::tests::mqtt5_1_5_2_a_two_byte_integer_is_big_endian`,
`mqtt5_1_5_4_a_string_is_its_length_and_its_utf8_octets`, `mqtt5_1_5_utf8_string_starts_with_be_length`,
`mqtt5_1_5_utf8_string_decode_invalid_utf8`, `mqtt5_1_5_utf8_string_decode_truncated`,
`mqtt5_1_5_6_binary_data_is_its_length_and_its_octets`; `vbi::tests::*` (10 Tests, alle Boundaries
0/127/128/16383/16384/2097151/2097152/MAX, 5-byte-Reject).

**Status:** done

---

## §2 MQTT Control Packet Format

### §2.1 Structure of an MQTT Control Packet

**Spec:** §2.1, S. 19-21 — Fixed Header (Type-Nibble + Flags-Nibble +
Remaining Length VBI), Variable Header, Payload.

**Repo:** `crates/mqtt-bridge/src/packet.rs::FixedHeader`.

**Tests:** `packet::tests::mqtt5_2_1_publish_header_decodes_dup_qos_retain_flags`,
`codec::tests::mqtt5_2_1_fixed_header_first_byte_layout_for_publish`.

**Status:** done

### §2.1.2 Control Packet Type (Table 2-1)

**Spec:** §2.1.2 — 15 Packet-Types (CONNECT=1, ..., AUTH=15).

**Repo:** `crates/mqtt-bridge/src/packet.rs::ControlPacketType` mit
allen 15 Werten.

**Tests:** `packet::tests::mqtt5_2_1_2_the_packet_types_are_the_values_of_table_2_1`,
`mqtt5_2_1_2_packet_type_zero_is_reserved`.

**Status:** done

### §2.1.4 Remaining Length

**Spec:** §2.1.4 — VBI; Bytes nach Fixed Header bis Ende des Pakets.

**Repo:** Cross-Ref VBI-Modul.

**Tests:** `codec::tests::mqtt5_2_1_4_large_payload_encodes_multibyte_remaining_length`,
`mqtt5_2_1_4_truncated_remaining_length_decode_fails`.

**Status:** done

### §2.2 Variable Header

**Spec:** §2.2, S. 21 — Per-Packet-Type Variable-Header-Inhalt.

**Repo:** PUBLISH-Variable-Header in
`crates/mqtt-bridge/src/codec.rs::encode_publish/decode_publish`.

**Tests:** `codec::tests::mqtt5_3_3_2_2_a_packet_identifier_only_above_qos_0`,
`codec::tests::mqtt5_3_3_2_the_publish_of_figure_3_9`.

**Status:** done — für PUBLISH; weitere Packet-Types analog
implementierbar.

### §2.2.2 Properties

**Spec:** §2.2.2, S. 23-25 — VBI-Length-prefixed Property-Block;
Property-Identifier (Spec Table 2-4) + Wert.

**Repo:** `crates/mqtt-bridge/src/properties.rs::{PropertyId, Property}`
mit 15 named-IDs + `Other(u32)`-Variante. Codec serialisiert
Property-Block-Body als opaque Bytes mit VBI-Length-Prefix.

**Tests:** `properties::tests::mqtt5_2_2_2_each_named_property_has_its_identifier_of_table_2_4`,
`mqtt5_2_2_2_property_id_unknown_yields_other_variant`,
`codec::tests::mqtt5_3_3_2_3_the_properties_follow_their_length`.

**Status:** done — Identifier-Registry + Wire-Length-Prefix +
`PropertyValueKind`-Tabelle nach Spec §2.2.2.2 Table 2-4 mit
Decode-Helpers (`decode_two_byte_int`, `decode_four_byte_int`,
`decode_utf8_string`, `decode_binary_data`) und
`PropertyId::value_kind()` als zentralem Dispatch.

---

## §3 MQTT Control Packets

### §3.1 CONNECT

**Spec:** §3.1, S. 30-46 — Verbindungsaufbau mit Will/Properties/
Authentication.

**Repo:** `crates/mqtt-bridge/src/control_packets.rs::{encode_connect_body, decode_connect_body}` (`ConnectBody`).

**Tests:** `control_packets::tests::mqtt5_3_1_3_without_will_and_credentials_the_payload_is_the_client_identifier`,
`control_packets::tests::mqtt5_3_1_2_12_the_connect_of_figure_3_6_and_its_payload_in_the_order_of_3_1_3`.

**Status:** done

### §3.2 CONNACK

**Spec:** §3.2 — Connect Acknowledgment.

**Repo:** `control_packets::{encode_connack_body, decode_connack_body}` (`ConnackBody`).

**Tests:** `control_packets::tests::mqtt5_3_2_2_a_connack_is_flags_reason_code_and_properties`.

**Status:** done

### §3.3 PUBLISH — Voll abgedeckt

**Spec:** §3.3, S. 48-58 — Vollständige PUBLISH-Packet-Spec mit
DUP/QoS/RETAIN-Flags, Topic-Name, Packet-Identifier (bei QoS>0),
Properties, Payload.

**Repo:** `crates/mqtt-bridge/src/codec.rs::encode_publish/
decode_publish` voll implementiert; QoS-3 wird abgelehnt
(MalformedPacket), Packet-Identifier-Pflicht bei QoS>0 enforced.

**Tests:** `codec::tests::mqtt5_3_3_2_2_a_packet_identifier_only_above_qos_0`,
`mqtt5_3_3_2_the_publish_of_figure_3_9`, `mqtt5_3_3_invalid_qos_3_rejected_on_encode`,
`mqtt5_3_3_missing_packet_id_at_qos1_rejected`,
`mqtt5_3_3_wrong_packet_type_rejected_on_decode`,
`mqtt5_2_1_fixed_header_first_byte_layout_for_publish`,
`mqtt5_3_3_2_3_the_properties_follow_their_length`,
`mqtt5_2_1_4_truncated_remaining_length_decode_fails`,
`mqtt5_3_3_empty_input_decode_fails`,
`mqtt5_2_1_4_large_payload_encodes_multibyte_remaining_length`.

**Status:** done

### §3.4 PUBACK

**Spec:** §3.4 — Acknowledgment für QoS-1-PUBLISH.

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`
(gemeinsame `AckBody`-Struktur).

**Tests:** `control_packets::tests::mqtt5_3_4_2_a_puback_is_packet_id_reason_code_and_properties`.

**Status:** done

### §3.5 PUBREC

**Spec:** §3.5 — QoS-2 Receive-Acknowledgment (Schritt 1 des
4-Way-Handshake).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`
(gleiche AckBody-Struktur).

**Tests:** `control_packets::tests::mqtt5_3_4_2_a_puback_is_packet_id_reason_code_and_properties`.

**Status:** done

### §3.6 PUBREL

**Spec:** §3.6 — QoS-2 Publish-Release (Schritt 2).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`.

**Tests:** `control_packets::tests::mqtt5_3_4_2_a_puback_is_packet_id_reason_code_and_properties`.

**Status:** done

### §3.7 PUBCOMP

**Spec:** §3.7 — QoS-2 Publish-Complete (Schritt 3, finaler ACK).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`.

**Tests:** `control_packets::tests::mqtt5_3_4_2_a_puback_is_packet_id_reason_code_and_properties`.

**Status:** done

### §3.8 SUBSCRIBE

**Spec:** §3.8 — Client-zu-Server-Subscription-Request mit
Topic-Filter-Liste.

**Repo:** `control_packets::{encode_subscribe_body,
decode_subscribe_body}`.

**Tests:** `control_packets::tests::mqtt5_3_8_the_subscribe_of_figures_3_19_and_3_21`.

**Status:** done

### §3.9 SUBACK

**Spec:** §3.9 — Server-zu-Client SUBSCRIBE-Acknowledgment mit
Reason-Codes pro Subscription.

**Repo:** `control_packets::{encode_suback_body,
decode_suback_body}`.

**Tests:** `control_packets::tests::mqtt5_3_9_a_suback_is_packet_id_properties_and_a_reason_code_per_filter`.

**Status:** done

### §3.10 UNSUBSCRIBE

**Spec:** §3.10 — Client-Unsubscribe-Request.

**Repo:** `control_packets::{encode_unsubscribe_body,
decode_unsubscribe_body}`.

**Tests:** `control_packets::tests::mqtt5_3_10_the_unsubscribe_payload_of_figure_3_30`.

**Status:** done

### §3.11 UNSUBACK

**Spec:** §3.11 — Server-UNSUBSCRIBE-Acknowledgment mit Reason-Codes.

**Repo:** `control_packets::{encode_unsuback_body,
decode_unsuback_body}`.

**Tests:** `control_packets::tests::mqtt5_3_11_an_unsuback_is_packet_id_properties_and_a_reason_code_per_filter`.

**Status:** done

### §3.12 PINGREQ

**Spec:** §3.12 — 2-byte Heartbeat (Type-Byte + Remaining=0).

**Repo:** Konstante `0xC0`; Encode trivial (1 Byte 0xC0 + VBI(0)).

**Tests:** `packet::tests::mqtt5_2_1_2_the_packet_types_are_the_values_of_table_2_1`.

**Status:** done

### §3.13 PINGRESP

**Spec:** §3.13 — 2-byte Heartbeat-Reply (Type-Byte + Remaining=0).

**Repo:** Konstante `0xD0`; Encode trivial (1 Byte 0xD0 + VBI(0)).

**Tests:** `packet::tests::mqtt5_2_1_2_the_packet_types_are_the_values_of_table_2_1`.

**Status:** done

### §3.14 DISCONNECT

**Spec:** §3.14 — Verbindungsabbau (mit Reason-Code in 5.0 neu).

**Repo:** `control_packets::{encode_disconnect_body,
decode_disconnect_body}`.

**Tests:** `control_packets::tests::mqtt5_3_14_2_the_disconnect_of_figure_3_24`.

**Status:** done

### §3.15 AUTH

**Spec:** §3.15 — Erweiterte Authentifizierung (neu in 5.0,
multi-step SASL-like challenge/response).

**Repo:** `control_packets::{encode_auth_body, decode_auth_body}`.

**Tests:** `control_packets::tests::mqtt5_3_15_2_an_auth_is_reason_code_and_properties`.

**Status:** done

---

## §4 Operational Behavior

### §4.1-§4.13 Storing State + Network Connections + QoS Levels

**Spec:** §4 — Session-State, Topic-Filter-Matching, QoS-Protokoll-
Flow, Keep-Alive-Timer, Message-Ordering, etc.

**Repo:** `crates/mqtt-bridge/src/broker.rs::{Broker, Session,
Subscription, RetainedMessage, DeliveryEnvelope, QoS, Will}` mit
`connect`/`disconnect`/`subscribe`/`retained_for`/Packet-ID-Tracking.
Topic-Filter-Matching in `crates/mqtt-bridge/src/topic_filter.rs`.
Keep-Alive-Timer als Wall-Clock-Tick in
`crates/mqtt-bridge/src/keep_alive.rs::KeepAliveTracker` mit
`record_packet`/`is_expired`/`remaining` und Spec-§3.1.2.10-konformer
1.5×-Period-Expiry-Regel.

**Tests:** `keep_alive::tests::mqtt5_4_1_keep_alive_zero_is_disabled`,
`keep_alive::tests::mqtt5_4_1_fresh_tracker_not_expired`,
`keep_alive::tests::mqtt5_4_1_record_packet_resets_timer`,
`keep_alive::tests::mqtt5_4_1_small_keep_alive_expires_after_1_5x`,
`keep_alive::tests::mqtt5_4_1_remaining_decreases_over_time`;
`broker::tests::mqtt5_4_1_the_qos_levels_are_0_1_and_2`; `topic_filter::tests::*`.

**Status:** done — Session-Manager, Subscription-Tree, Retained-
Messages, Will-Handling, QoS-Levels (0/1/2) + Keep-Alive-Timer
spec-konform implementiert.

### §4 Standalone Broker-Server (Netz-Exposition der Broker-Logik)

**Spec:** §4 + §7 (Conformance-Target *MQTT-Server*) — ein lauschender
Broker akzeptiert Client-Verbindungen, handelt CONNECT/CONNACK, verteilt
PUBLISH an Subscriber, fährt den QoS-0/1/2-Protokoll-Flow.

**Repo:** `crates/mqtt-bridge/src/server.rs::{MqttBrokerServer,
ServerHandle}` — `TcpListener` + accept-Loop, pro Verbindung Reader-/
Writer-Thread (event-driven via mpsc-Channel), verdrahtet die
`crates/mqtt-bridge/src/broker.rs`-Engine: CONNECT/CONNACK, SUBSCRIBE/SUBACK (+ Retained-Replay
§3.3.1.3), PUBLISH-Fanout, QoS 0/1/2 (Exactly-Once liefert auf PUBREL,
§4.3.3), UNSUBSCRIBE/UNSUBACK, PINGREQ/PINGRESP, Will auf abnormalem
Disconnect (§3.1.2.5). Protokoll-Version pro Client aus dem CONNECT-
Protocol-Level ausgehandelt — 5.0 und 3.1.1 am selben Broker.
`crates/mqtt-bridge/src/net.rs::{MqttClient, read_packet}` — TCP-Framing
+ schlanker version-aware Client.

**Tests:** `crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_publish_subscribe_qos1`,
`mqtt5_4_retained_message_delivered_to_late_subscriber`, `mqtt5_4_wildcard_plus_and_hash`,
`mqtt5_4_qos2_exactly_once_delivery`, `mqtt5_4_cross_version_v5_publishes_v311_subscribes`,
`mqtt5_4_cross_version_v311_publishes_v5_subscribes`,
`mqtt5_4_pingreq_is_answered_with_a_bodyless_pingresp_within_the_keep_alive`; Live
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_v5_clients_through_zerodds_broker`,
`mqtt5_4_mosquitto_v311_clients_through_zerodds_broker`,
`mqtt5_4_zerodds_client_through_zerodds_broker_cross_version`,
`mqtt5_4_zerodds_311_client_receives_from_mosquitto`,
`mqtt5_4_mosquitto_311_client_receives_from_zerodds`.

**Status:** done

---

## §5 Security

### §5 Security Considerations

**Spec:** §5 — Security-Hinweise (TLS, Authentication, Authorization,
Privacy); informativ, kein Wire-Mapping.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Kapitel 5 trägt in der Spec die Kennzeichnung „(non-normative)“.

## §6 Web Socket Support

### §6 WebSocket-Transport für MQTT

**Spec:** §6 — "MQTT Control Packets MUST be sent in WebSocket binary data frames. If any other type of data frame is received the recipient MUST close the Network Connection [MQTT-6.0.0-1]." "The receiver MUST NOT assume that MQTT Control Packets are aligned on WebSocket frame boundaries [MQTT-6.0.0-2]." "The Client MUST include mqtt in the list of WebSocket Sub Protocols it offers [MQTT-6.0.0-3]." "The WebSocket Subprotocol name selected and returned by the Server MUST be mqtt [MQTT-6.0.0-4]."

**Repo:** `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`WsStream`, Subprotokoll `mqtt`), `crates/mqtt-bridge/src/daemon/client.rs`

**Tests:** `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_ws_broker_url_carries_mqtt_both_ways_through_mosquitto`, `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate`, `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_a_ws_url_on_a_plain_listener_fails_the_handshake`

**Status:** partial — die Client-Seite (`ws://`, `wss://` im Daemon) hält die vier Regeln und ist gegen Mosquitto getestet (die Tests sind `#[ignore]`, sie brauchen Mosquitto). Der Server (`crates/mqtt-bridge/src/server.rs`) nimmt keine WebSocket-Verbindung an; eigenes Item.

### §6 MQTT-Server über WebSocket

**Spec:** §6 — [MQTT-6.0.0-1], [MQTT-6.0.0-2] und [MQTT-6.0.0-4] auf der Seite des Servers: binäre Frames, Pakete über Frame-Grenzen hinweg, Subprotokoll `mqtt` auswählen.

**Repo:** —

**Tests:** —

**Status:** open — `MqttBrokerServer` (`crates/mqtt-bridge/src/server.rs`) hat nur einen TCP-Listener.

## §7 Conformance

### §7 Conformance Targets

**Spec:** §7 — Drei Conformance-Targets: MQTT-Client, MQTT-Server,
MQTT-Server-with-MQTT-Bridge-Functionality.

**Repo:** Wire-Codec + Broker-Logik + Netz-Server im `mqtt-bridge`-Crate.
Die Targets *MQTT-Client* (`net.rs::MqttClient`) und *MQTT-Server*
(`crates/mqtt-bridge/src/server.rs::MqttBrokerServer`) sind realisiert und gegen die Eclipse-
Mosquitto-Referenz interop-getestet (`tests/mosquitto_interop_e2e.rs`).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs`.

**Status:** n/a (informative) — benennt die Konformitätsziele Client, Server und Server mit Bridge-Funktion; die Anforderungen sind die Items dieses Dokuments.

---

## Audit-Status

23 done / 1 partial / 1 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document mqtt-5.0.md`):

* `cargo nextest run --profile merge -p zerodds-mqtt-bridge --all-targets` (Job `merge-rust-mqtt-bridge`) — 45 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor` (Job `integration-bridges-vendor`) — 7 Tests grün.

52 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `mqtt-5.0.open.md`.
