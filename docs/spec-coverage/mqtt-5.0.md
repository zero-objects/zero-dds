# OASIS MQTT v5.0 — Spec-Coverage

**Spec:** [OASIS MQTT Version 5.0 — Standard, 07 March 2019 →](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html)

**Kontext:** MQTT 5.0 ist die OASIS-Pub-Sub-Spec für IoT/M2M.
ZeroDDS implementiert den Codec der Pakete (Spec §1.5 Data Types, §2.1
Fixed Header, §2.2 Variable Header + Properties, Kapitel 3) als pure-Rust
no_std+alloc Library, die Engine der Sessions eines Servers (Kapitel 4)
und einen Server im Netz, der die Aussagen der Kapitel 3, 4 und 6 über
den Server hält.

Implementation:

- `crates/mqtt-bridge/` — Codec, Engine der Sessions (`broker.rs`),
  Server (`server/`), Client (`net.rs`, `daemon/client.rs`), DDS-Bridge.

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

**Spec:** §2.2.2, S. 23-25 — Block der Properties mit Längenpräfix als
Variable Byte Integer; Property Identifier (Spec Table 2-4) + Wert.

**Repo:** `crates/mqtt-bridge/src/properties.rs::{PropertyId, Property, PropertyValueKind}`:
eine Variante je Identifier der Tabelle 2-4 (27), `PropertyId::value_kind()` gibt
den Datentyp der Tabelle zurück, für einen Identifier außerhalb der Tabelle
(`Other(u32)`) `None`. `crates/mqtt-bridge/src/control_packets.rs::property_data_type`
liest dieselbe Tabelle; `property_value` weist einen Block der Properties mit
einem unbekannten Identifier ab, weil dessen Wert nicht abgrenzbar ist
(§2.2.2.2). Der Codec serialisiert den Block der Properties als Bytes mit
einem Längenpräfix als Variable Byte Integer.
`crates/mqtt-bridge/src/property_set.rs::{PropertySet, allowed_in}` liest einen
Block typisiert und prüft ihn gegen die Paketspalte der Tabelle 2-4: ein
Identifier, den die Tabelle für das Paket nicht nennt, ist ein Malformed
Packet; eine Property außer User Property und Subscription Identifier doppelt
und ein verbotener Wert sind ein Protocol Error, ein Topic Alias 0 mit dem
genaueren Reason Code 0x94 (§3.3.2.3.4); `PropertySet::encode` schreibt in
der Reihenfolge der Tabelle, User Properties in ihrer Reihenfolge. Der Server
(`crates/mqtt-bridge/src/server/`) liest und schreibt jeden Block der
Properties darüber.

**Tests:** `properties::tests::mqtt5_2_2_2_all_27_identifiers_of_table_2_4_and_their_types`,
`properties::tests::mqtt5_2_2_2_an_identifier_outside_table_2_4_has_no_value_type`,
`codec::tests::mqtt5_3_3_2_3_the_properties_follow_their_length`,
`property_set::tests::mqtt5_2_2_2_a_set_round_trips_and_keeps_the_order_of_user_properties`,
`property_set::tests::mqtt5_2_2_2_an_identifier_not_listed_for_the_packet_is_malformed`,
`property_set::tests::mqtt5_2_2_2_a_property_twice_is_a_protocol_error_user_property_is_not`,
`property_set::tests::mqtt5_2_2_2_forbidden_values_are_protocol_errors`,
`property_set::tests::mqtt5_1_5_4_a_string_with_u0000_or_bad_utf8_and_a_cut_value_are_malformed`.

**Status:** done — alle 27 Identifier der Tabelle 2-4 mit Wert und Datentyp;
ein Identifier außerhalb der Tabelle hat keinen Datentyp.

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

**Spec:** §4 — Session State, Abgleich der Topic Filter, Abläufe der
QoS-Stufen, Keep Alive, Reihenfolge der Nachrichten.

**Repo:** `crates/mqtt-bridge/src/broker.rs::{Broker, Session,
Subscription, RetainedMessage, DeliveryEnvelope, AppMessage,
MessageProperties, QoS, Will}` — die Engine der Sessions, ohne Netz; die
Zeit sind Millisekunden des Aufrufers. Sie hält Session Expiry Interval
und Session Present, die Nachrichten vor der Übertragung (QoS 0 nur bei
offener Verbindung; ohne Vorgabe unbegrenzt, mit einer Grenze je Session
wird an der vollen Warteschlange QoS 0 nicht gehalten und eine QoS-1-
oder QoS-2-Nachricht meldet die Session zum Beenden,
`Delivered::over_limit`, `Broker::end_session`), die QoS-1- und
QoS-2-Zustellungen in Sendereihenfolge,
die empfangenen QoS-2-Packet-Identifier bis zum PUBREL, den Will mit
Will Delay Interval und Will Properties. Weiterleitung: eine Kopie je
Session mit der höchsten QoS und allen Subscription Identifiers, No
Local, Retain As Published, Retain Handling, Shared Subscriptions,
Message Expiry, Send Quota (§4.9), Wiederholung nur beim
Wiederverbinden innerhalb des Receive Maximum der neuen Verbindung
(§4.4). Der Abgleich der Topic Filter steht in
`crates/mqtt-bridge/src/topic_filter.rs`, Keep Alive in
`crates/mqtt-bridge/src/keep_alive.rs::KeepAliveTracker` (1,5 × Keep
Alive). Das Netz dazu beschreiben die folgenden Items.

**Tests:** `keep_alive::tests::mqtt5_4_1_keep_alive_zero_is_disabled`,
`keep_alive::tests::mqtt5_4_1_fresh_tracker_not_expired`,
`keep_alive::tests::mqtt5_4_1_record_packet_resets_timer`,
`keep_alive::tests::mqtt5_4_1_small_keep_alive_expires_after_1_5x`,
`keep_alive::tests::mqtt5_4_1_remaining_decreases_over_time`;
`broker::tests::mqtt5_4_1_the_qos_levels_are_0_1_and_2`,
`broker::tests::mqtt5_4_3_2_a_packet_identifier_in_flight_is_not_assigned_again`,
`broker::tests::mqtt5_3_1_2_4_clean_start_discards_and_session_present_reports_a_resumed_session`,
`broker::tests::mqtt5_4_1_a_session_ends_when_its_expiry_interval_has_passed`,
`broker::tests::mqtt5_3_1_2_8_a_delayed_will_follows_its_delay_or_the_end_of_the_session`,
`broker::tests::mqtt5_3_3_4_overlapping_subscriptions_give_one_copy_with_the_highest_qos_and_every_identifier`,
`broker::tests::mqtt5_3_8_3_1_no_local_and_retain_as_published`,
`broker::tests::mqtt5_3_3_1_3_retain_handling_0_1_2`,
`broker::tests::mqtt5_4_8_2_a_shared_subscription_sends_each_message_to_one_session`,
`broker::tests::mqtt5_4_9_the_send_quota_holds_back_qos_1_and_2_not_qos_0`,
`broker::tests::mqtt5_4_4_resend_is_in_order_and_a_failed_pubrec_discards`,
`broker::tests::mqtt5_4_3_3_a_duplicate_qos2_publish_is_not_a_new_message_until_pubrel`,
`broker::tests::mqtt5_3_3_2_3_3_an_expired_message_is_deleted_before_it_is_sent`,
`broker::tests::mqtt5_4_1_qos0_is_not_kept_for_an_offline_session_and_a_full_queue_ends_the_session`,
`broker::tests::mqtt5_4_1_at_a_full_queue_qos0_is_not_kept_and_the_ended_session_publishes_its_will`,
`broker::tests::mqtt5_4_9_a_reconnect_resends_within_the_new_receive_maximum`;
`topic_filter::tests::*`.

**Status:** done — die Engine trägt den Session State von §4.1 und die
Weiterleitung; was der Server im Netz damit tut, belegen die Items ab
„§4 Standalone Broker-Server“.

### §4 Standalone Broker-Server (Netz-Exposition der Broker-Logik)

**Spec:** §4 + §7.1.1 (Konformitätsziel *MQTT Server*) — ein lauschender
Server nimmt Verbindungen von Clients an und hält die Aussagen der
Kapitel 3 und 4 über den Server. Die Aussagen stehen gruppiert in den Items „— Server“
unten.

**Repo:** `crates/mqtt-bridge/src/server/mod.rs::{MqttBrokerServer,
ServerHandle, ServerConfig, ClientStream, Authenticator}` — ein
`TcpListener` und eine Schleife, die Verbindungen annimmt, daneben die
Listener für WebSocket, TLS und WebSocket über TLS des Items „§6
MQTT-Server über WebSocket“; je Verbindung
ein Thread, der liest (das Protokoll,
`crates/mqtt-bridge/src/server/conn.rs`), und einer, der als einziger in
den Socket schreibt. Der Session State liegt in der Engine
`crates/mqtt-bridge/src/broker.rs` hinter einer Sperre; der Thread
`housekeeping` veröffentlicht verzögerte Wills, beendet abgelaufene
Sessions und löscht abgelaufene Nachrichten. `ServerConfig` enthält, was
der Server selbst entscheidet: Receive Maximum, Maximum Packet Size,
Topic Alias Maximum, Maximum QoS, Retain Available, Wildcard
Subscription Available, Subscription Identifiers Available, Shared
Subscription Available, Server Keep Alive, das längste Session Expiry
Interval, Response Information, Umleitung, die Grenze der Nachrichten je
Session (`max_queued_messages`, ohne Vorgabe unbegrenzt) und den
`Authenticator`. Die Protokollversion
kommt je Client aus dem Protocol Level, 5.0 und 3.1.1 am selben Server.
`ClientStream` nimmt Verbindungen anderer Transporte an
(`ServerHandle::serve`). `crates/mqtt-bridge/src/server/inbound.rs` liest
jedes Paket eines Clients streng und gibt bei einem Fehler den Reason
Code von §4.13 zurück. `crates/mqtt-bridge/src/net.rs::{MqttClient,
read_packet_within}` — TCP-Framing mit Größengrenze und ein schlanker
Client.

**Tests:** `crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_publish_subscribe_qos1`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_retained_message_delivered_to_late_subscriber`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_wildcard_plus_and_hash`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_qos2_exactly_once_delivery`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_cross_version_v5_publishes_v311_subscribes`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_cross_version_v311_publishes_v5_subscribes`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_pingreq_is_answered_with_a_bodyless_pingresp_within_the_keep_alive`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_2_a_connection_of_another_transport_is_served_through_client_stream`;
gegen Mosquitto
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_v5_clients_through_zerodds_broker`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_v311_clients_through_zerodds_broker`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_zerodds_client_through_zerodds_broker_cross_version`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_zerodds_311_client_receives_from_mosquitto`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_311_client_receives_from_zerodds`.

**Status:** done — die Tests gegen Mosquitto sind `#[ignore]` und laufen
im wöchentlichen Job `integration-bridges-vendor`; auf dem Linux-Testhost
gegen Mosquitto 2.0.21 grün.

### §3.1 CONNECT — Server

**Spec:** [MQTT-3.1.0-1], [MQTT-3.1.0-2], [MQTT-3.1.2-1] bis
[MQTT-3.1.2-15], [MQTT-3.1.2-22], [MQTT-3.1.2-23], [MQTT-3.1.2-28],
[MQTT-3.1.3-5] bis [MQTT-3.1.3-10], [MQTT-3.1.4-1] bis [MQTT-3.1.4-6].

**Repo:** `crates/mqtt-bridge/src/server/inbound.rs::parse_connect`
(Protocol Name und Level, reserviertes Flag, Will Flag, Will QoS und Will
Retain, User Name Flag und Password Flag, die Blöcke der Properties,
Reihenfolge der Felder, kein Oktett nach dem letzten Feld; 0x84 im
Format, das der Client lesen kann);
`crates/mqtt-bridge/src/server/conn.rs::Connection::open` (das erste
Paket ist CONNECT, Assigned Client Identifier, Übernahme mit DISCONNECT
0x8E, Clean Start, Will mit Will Delay Interval und Will Properties, Keep
Alive über `KeepAliveTracker`, Will QoS über Maximum QoS mit 0x9B, Will
Retain ohne Retain Available mit 0x9A), `Connection::handle` (ein zweiter
CONNECT ist ein Protocol Error), `Connection::own_session` (nach der
Übernahme oder dem Ende der Session mit 0x97 behandelt die alte Verbindung
kein Paket mehr gegen die Session: was sie noch liest, beendet sie).

**Tests:** `server::inbound::tests::mqtt5_3_1_2_the_protocol_name_level_and_reserved_flag_are_checked`,
`server::inbound::tests::mqtt5_3_1_2_5_will_flags_without_a_will_and_trailing_octets_are_malformed`,
`server::inbound::tests::mqtt5_3_1_3_the_3_1_1_return_codes_of_a_refusal`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_0_2_a_second_connect_is_a_protocol_error`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_2_an_unsupported_protocol_version_gets_0x84_and_the_close`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_3_the_reserved_connect_flag_gets_0x81`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_3_1_an_empty_client_id_gets_an_assigned_client_identifier`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_4_3_a_second_connection_takes_the_session_over_with_0x8e`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_4_3_what_the_replaced_connection_still_reads_does_not_reach_the_session`
(die alte Verbindung sendet nach ihrem DISCONNECT 0x8E ein SUBSCRIBE und
ein DISCONNECT mit Session Expiry Interval 0 über einen Stream, den der
Server nach dem Schließen weiter liest: die Session der neuen Verbindung
bekommt die Subscription nicht und behält ihr Session Expiry Interval),
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_10_keep_alive_closes_after_one_and_a_half_periods_and_publishes_the_will`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_3_2_2_a_delayed_will_waits_and_a_reconnect_cancels_it`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_28_response_information_only_on_request`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_0_1_session_present_and_clean_session_of_3_1_1_clients`.

**Status:** done — grün auf dem Linux-Testrechner. Vorher behandelte die
alte Verbindung nach der Übernahme die Pakete, die ihr Reader noch hatte,
gegen die Session der neuen: ohne die Prüfung in `own_session` bekommt die
neue Verbindung im Test die Nachricht auf `old/t` (gemessen mit einer
Mutation).

### §3.2 CONNACK — Server

**Spec:** [MQTT-3.2.0-1], [MQTT-3.2.0-2], [MQTT-3.2.2-1] bis
[MQTT-3.2.2-3], [MQTT-3.2.2-6] bis [MQTT-3.2.2-10], [MQTT-3.2.2-12],
[MQTT-3.2.2-13], [MQTT-3.2.2-16], [MQTT-3.2.2-19] bis [MQTT-3.2.2-22].

**Repo:** `crates/mqtt-bridge/src/server/conn.rs::Connection::open` — der
CONNACK geht vor jedem anderen Paket außer AUTH hinaus, einmal je
Verbindung; Session Present nach §3.2.2.1.1, 0 bei einem Reason Code
ungleich 0; die Properties aus `ServerConfig` (Session Expiry Interval,
wenn der Server einen anderen Wert nimmt; Receive Maximum; Maximum QoS;
Retain Available; Maximum Packet Size; Assigned Client Identifier; Topic
Alias Maximum; Wildcard Subscription Available, Subscription Identifier
Available und Shared Subscription Available; Server Keep Alive; Response
Information; Authentication Method und Data). Ab Reason Code 0x80 wird
die Verbindung geschlossen. Server Keep Alive ersetzt nur den Keep Alive
eines 5.0-Clients, der ihn im CONNACK erhält; ein 3.1.1-Client behält
seinen ([MQTT-3.2.2-22]). Was der CONNACK als nicht verfügbar
ankündigt, beendet bei einem 5.0-Client die Verbindung: eine Wildcard
Subscription mit DISCONNECT 0xA2, eine Shared Subscription mit 0x9E, ein
Subscription Identifier mit 0xA1 (§3.2.2.3.11 bis §3.2.2.3.13). `crates/mqtt-bridge/src/server/inbound.rs::refusal`
bildet die Reason Codes auf die Return Codes von 3.1.1 ab.

**Tests:** `crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_connack_carries_the_server_limits_and_session_present_follows_clean_start`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_features_the_server_does_not_offer`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_14_the_server_keep_alive_replaces_the_clients`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_22_a_3_1_1_client_keeps_its_own_keep_alive_beside_a_server_keep_alive`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_2_a_session_expiry_above_the_servers_maximum_is_answered`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_3_1_an_empty_client_id_gets_an_assigned_client_identifier`.

**Status:** done — Reason String und User Property sendet der Server im
CONNACK nicht; [MQTT-3.2.2-19] und [MQTT-3.2.2-20] gelten damit ohne
eigenen Test.

### §3.3 PUBLISH — Server

**Spec:** [MQTT-3.3.1-1] bis [MQTT-3.3.1-13], [MQTT-3.3.2-1] bis
[MQTT-3.3.2-8], [MQTT-3.3.2-11] bis [MQTT-3.3.2-20], [MQTT-3.3.4-1] bis
[MQTT-3.3.4-6], [MQTT-3.3.4-9], [MQTT-3.3.4-10], [MQTT-3.1.2-24] bis
[MQTT-3.1.2-27]; §3.3.2.3.2 Payload Format Indicator.

**Repo:** `crates/mqtt-bridge/src/server/inbound.rs::parse_publish`
(QoS 3, DUP bei QoS 0, Packet Identifier 0); `crates/mqtt-bridge/src/server/conn.rs::Connection::on_publish`
(Topic Name ohne Wildcard, Topic Alias des Clients bis zum Topic Alias
Maximum des Servers, ein Topic Alias 0 oder darüber mit 0x94 — 0 prüft
`crates/mqtt-bridge/src/property_set.rs::PropertySet::parse` —, keine
Subscription Identifier vom Client, Receive
Maximum des Servers, Payload Format Indicator mit 0x99, PUBACK und
PUBREC mit 0x10 ohne passenden Subscriber), `publish_bytes` (Properties
unverändert, Message Expiry Interval als Rest, Subscription Identifiers,
Topic Alias zum Client bis zu dessen Maximum, DUP nur bei Wiederholung,
eine Nachricht über der Maximum Packet Size des Clients wird verworfen
und gilt als gesendet), `pump` (Receive Maximum des Clients).

**Tests:** `server::inbound::tests::mqtt5_3_3_1_publish_flags_and_packet_identifier_are_checked`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_2_3_the_properties_of_a_message_reach_the_subscriber_unaltered`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_2_3_3_an_expired_message_is_deleted_and_the_rest_carries_what_is_left`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_1_3_retain_handling_and_retain_as_published`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_4_overlapping_subscriptions_give_one_copy_with_every_identifier`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_4_6_a_subscription_identifier_from_the_client_is_a_protocol_error`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_2_2_a_topic_name_with_a_wildcard_gets_0x90`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_2_3_2_a_payload_that_is_not_utf8_gets_0x99`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_2_3_4_topic_aliases_from_the_client`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_26_topic_aliases_to_the_client_stay_within_its_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_24_a_message_over_the_clients_maximum_packet_size_is_not_sent`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_4_a_client_over_the_servers_receive_maximum_gets_0x93`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_9_the_server_sends_no_more_than_the_clients_receive_maximum`;
gegen Mosquitto
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_3_3_2_3_mosquitto_v5_properties_cross_the_zerodds_broker_unaltered`.

**Status:** done — der Test gegen Mosquitto ist `#[ignore]` und läuft im
wöchentlichen Job `integration-bridges-vendor`; auf dem Linux-Testhost
gegen Mosquitto 2.0.21 grün.

### §3.4-§3.7 PUBACK, PUBREC, PUBREL, PUBCOMP — Server

**Spec:** [MQTT-3.4.2-1] bis [MQTT-3.4.2-3], [MQTT-3.5.2-1] bis
[MQTT-3.5.2-3], [MQTT-3.6.1-1], [MQTT-3.6.2-1] bis [MQTT-3.6.2-3],
[MQTT-3.7.2-1] bis [MQTT-3.7.2-3].

**Repo:** `crates/mqtt-bridge/src/server/inbound.rs::parse_ack`
(Flags, PUBREL mit 0b0010, Kurzformen, Reason Codes der Tabelle des
Pakets); `crates/mqtt-bridge/src/server/conn.rs::Connection::send_ack`
und `send_fitted` (ein Reason String nur bei einem Fehler, ohne
Properties, wenn sie die Maximum Packet Size des Clients überschritten);
PUBREL und PUBCOMP mit 0x92 für einen unbekannten Packet Identifier.

**Tests:** `server::inbound::tests::mqtt5_3_4_2_acks_short_forms_flags_and_reason_codes`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_6_1_a_pubrel_with_flags_0_is_malformed`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_4_2_1_a_puback_says_0x10_without_matching_subscribers`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_6_2_2_a_reason_string_beyond_the_clients_maximum_packet_size_is_left_out`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_3_3_a_duplicate_qos2_publish_is_delivered_once`.

**Status:** done

### §3.8-§3.11 SUBSCRIBE, SUBACK, UNSUBSCRIBE, UNSUBACK — Server

**Spec:** [MQTT-3.8.1-1], [MQTT-3.8.3-1] bis [MQTT-3.8.3-5],
[MQTT-3.8.4-1] bis [MQTT-3.8.4-8], [MQTT-3.9.2-1], [MQTT-3.9.2-2],
[MQTT-3.9.3-1], [MQTT-3.9.3-2], [MQTT-3.10.1-1], [MQTT-3.10.3-1],
[MQTT-3.10.3-2], [MQTT-3.10.4-1] bis [MQTT-3.10.4-6], [MQTT-3.11.2-1],
[MQTT-3.11.2-2], [MQTT-3.11.3-1], [MQTT-3.11.3-2].

**Repo:** `crates/mqtt-bridge/src/server/inbound.rs::{parse_subscribe,
parse_unsubscribe}` (Flags 0b0010, mindestens ein Eintrag, reservierte
Bits, Maximum QoS und Retain Handling 3); `crates/mqtt-bridge/src/server/conn.rs::Connection::{on_subscribe,
on_unsubscribe}` (je Eintrag wie ein eigener SUBSCRIBE, Reason Code je
Topic Filter in der Reihenfolge, 0x8F für einen ungültigen Topic Filter,
No Local auf einer Shared Subscription als Protocol Error, gewährte QoS
bis Maximum QoS, SUBACK vor den Retained Messages, UNSUBACK mit 0x00
oder 0x11). Eine Wildcard Subscription oder Shared Subscription, die der
CONNACK als nicht verfügbar angekündigt hat, und ein Subscription
Identifier ohne Subscription Identifiers Available sind bei einem
5.0-Client ein Protocol Error: DISCONNECT 0xA2, 0x9E oder 0xA1 und das
Schließen ([MQTT-4.13.1-1]); ein 3.1.1-Client erhält 0x80 für den
Eintrag.

**Tests:** `server::inbound::tests::mqtt5_3_8_3_subscribe_flags_options_and_payload_are_checked`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_9_3_suback_and_unsuback_reason_codes_and_request_problem_information`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_8_3_5_reserved_subscription_option_bits_and_a_shared_no_local`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_8_3_3_no_local_keeps_a_message_from_its_publisher`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_1_3_retain_handling_and_retain_as_published`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_features_the_server_does_not_offer`.

**Status:** done

### §3.12-§3.15 PINGREQ, DISCONNECT, AUTH — Server

**Spec:** [MQTT-3.12.4-1], [MQTT-3.14.0-1], [MQTT-3.14.1-1],
[MQTT-3.14.2-1], [MQTT-3.14.2-2], [MQTT-3.14.4-1] bis [MQTT-3.14.4-3],
[MQTT-3.15.1-1], [MQTT-3.15.2-1]; §3.14.2.2.2 Session Expiry Interval im
DISCONNECT.

**Repo:** `crates/mqtt-bridge/src/server/conn.rs::Connection::handle`
(PINGRESP; DISCONNECT des Clients: 0x00 verwirft den Will, jeder andere
Code veröffentlicht ihn, ein Session Expiry Interval ungleich 0 ist ein
Protocol Error, wenn der CONNECT 0 verlangt hat — geprüft gegen den Wert
des CONNECT, nicht gegen den gewährten), `send_disconnect` (DISCONNECT
des Servers erst nach dem CONNACK, nie mit Session Expiry Interval;
DISCONNECT und Schließen gehen als eine Nachricht `Outbound::Disconnect`
an den Thread, der schreibt, und was danach eingereiht wird, schreibt er
nicht mehr), `ServerHandle::shutdown` (0x8B; eine Verbindung, deren
CONNECT erst danach kommt, erhält in `Connection::open` CONNACK 0x88
Server unavailable, unter 3.1.1 den Return Code 0x03, und wird
geschlossen: 0x8B ist kein CONNACK Reason Code, [MQTT-3.2.2-8]);
`crates/mqtt-bridge/src/server/inbound.rs::{parse_disconnect,
parse_auth}`.

**Tests:** `server::inbound::tests::mqtt5_3_4_2_acks_short_forms_flags_and_reason_codes`,
`server::conn::tests::mqtt5_3_14_4_1_nothing_follows_the_servers_disconnect`,
`server::conn::tests::mqtt5_3_14_4_1_a_3_1_1_client_is_closed_without_a_disconnect`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_pingreq_is_answered_with_a_bodyless_pingresp_within_the_keep_alive`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_14_4_disconnect_0x04_publishes_the_will_0x00_does_not`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_14_2_2_2_a_session_expiry_in_disconnect_after_0_in_connect_is_a_protocol_error`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_14_0_1_a_server_shutdown_sends_0x8b`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_2_a_connect_after_the_shutdown_gets_0x88_and_the_close`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_12_enhanced_authentication_and_re_authentication`.

**Status:** done — grün auf dem Linux-Testrechner. Vorher erhielt eine
Verbindung, die vor dem Herunterfahren angenommen war und ihren CONNECT
danach schickte, CONNACK 0x00 und blieb ohne Ende offen (im Code gelesen;
der Test oben scheitert mit dem alten Code, gemessen mit einer Mutation).

### §4.1 Session State — Server

**Spec:** [MQTT-4.1.0-1], [MQTT-4.1.0-2], [MQTT-3.1.2-23] — der Server
hält die Session über das Ende der Verbindung hinaus, solange das
Session Expiry Interval läuft, und verwirft sie danach.

**Repo:** `crates/mqtt-bridge/src/broker.rs::Broker::{connect_session,
disconnect_session, tick, next_deadline, deliver, end_session}`; der
Thread `housekeeping` in `crates/mqtt-bridge/src/server/mod.rs`;
`crates/mqtt-bridge/src/server/conn.rs::route`;
`ServerConfig::{maximum_session_expiry, max_queued_messages}`.

**Tests:** `broker::tests::mqtt5_4_1_a_session_ends_when_its_expiry_interval_has_passed`,
`broker::tests::mqtt5_4_1_qos0_is_not_kept_for_an_offline_session_and_a_full_queue_ends_the_session`,
`broker::tests::mqtt5_4_1_at_a_full_queue_qos0_is_not_kept_and_the_ended_session_publishes_its_will`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_1_a_session_ends_after_its_expiry_interval`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_1_a_full_queue_ends_the_session_with_0x97_and_drops_no_message_from_it`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_2_a_session_expiry_above_the_servers_maximum_is_answered`;
gegen Mosquitto
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_3_1_2_5_a_mosquitto_v5_session_gets_its_queued_message_at_reconnect`.

**Status:** done — QoS-0-Nachrichten für eine Session ohne Verbindung
hält der Server nicht (§4.1 erlaubt es, verlangt es nicht).
`ServerConfig::max_queued_messages` ist ohne Vorgabe unbegrenzt. Mit
einer Grenze hält der Server an der vollen Warteschlange eine
QoS-0-Nachricht nicht; eine QoS-1- oder QoS-2-Nachricht beendet die
Session: DISCONNECT 0x97 Quota exceeded an einen 5.0-Client, das
Schließen der Verbindung, danach verwirft der Server den Session State
und veröffentlicht den Will (§4.1.1, [MQTT-3.1.2-8]). Eine QoS-1- oder
QoS-2-Nachricht fehlt damit nie in einer Session, die weiterläuft.
Beides zählt `ServerHandle::queue_overflows`. Der Test gegen Mosquitto
ist `#[ignore]` (Job `integration-bridges-vendor`), auf dem
Linux-Testhost grün.

### §4.3-§4.6 QoS-Abläufe, Wiederholung, Empfang, Reihenfolge — Server

**Spec:** [MQTT-4.3.1-1], [MQTT-4.3.2-1] bis [MQTT-4.3.2-5],
[MQTT-4.3.3-1] bis [MQTT-4.3.3-13], [MQTT-4.4.0-1], [MQTT-4.4.0-2],
[MQTT-4.5.0-1], [MQTT-4.6.0-5], [MQTT-4.6.0-6].

**Repo:** `crates/mqtt-bridge/src/broker.rs::Broker::{take_sendable,
puback, pubrec, pubcomp, resend, qos2_received, qos2_released}` — ein
unbenutzter Packet Identifier je Zustellung, PUBREL nach PUBREC, kein
PUBLISH mehr nach dem PUBREL, PUBACK und PUBREC ab 0x80 gelten als
Bestätigung; eingehendes QoS 2 nach Methode B (Weiterleitung beim
PUBLISH, Packet Identifier bis zum PUBREL, ein Duplikat wird nicht noch
einmal weitergeleitet); wiederholt wird nur beim Wiederverbinden, in der
ursprünglichen Reihenfolge, mit DUP und dem ursprünglichen Packet
Identifier, die PUBLISH-Pakete innerhalb des Receive Maximum der neuen
Verbindung, die übrigen, sobald PUBACK oder PUBCOMP die Send Quota
freigeben (`InFlight::resend_pending`); eine Warteschlange je Session
hält die Reihenfolge je Topic. Eine Nachricht kommt in den Session State
jedes passenden Clients ([MQTT-4.5.0-1]); an einer vollen Warteschlange
endet die Session, siehe §4.1.

**Tests:** `broker::tests::mqtt5_4_4_resend_is_in_order_and_a_failed_pubrec_discards`,
`broker::tests::mqtt5_4_3_3_a_duplicate_qos2_publish_is_not_a_new_message_until_pubrel`,
`broker::tests::mqtt5_4_3_2_a_packet_identifier_in_flight_is_not_assigned_again`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_4_unacknowledged_messages_are_sent_again_at_reconnect_with_dup`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_9_a_reconnect_resends_no_more_than_the_new_receive_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_1_a_full_queue_ends_the_session_with_0x97_and_drops_no_message_from_it`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_3_3_a_duplicate_qos2_publish_is_delivered_once`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_6_messages_of_a_topic_arrive_in_the_order_they_were_published`.

**Status:** done

### §4.7-§4.8 Topic Names, Topic Filters, Shared Subscriptions — Server

**Spec:** [MQTT-4.7.0-1], [MQTT-4.7.1-1], [MQTT-4.7.1-2],
[MQTT-4.7.2-1], [MQTT-4.7.3-1] bis [MQTT-4.7.3-4], [MQTT-4.8.2-1] bis
[MQTT-4.8.2-6].

**Repo:** `crates/mqtt-bridge/src/topic_filter.rs`;
`crates/mqtt-bridge/src/broker.rs::{parse_shared_filter, Broker::subscribe_with}`
(`$share/{ShareName}/{filter}`, keine Retained Messages für eine Shared
Subscription, je Nachricht eine Session der Gruppe, eine verbundene
zuerst, reihum); die Zustellung an die gewählte Session läuft über deren
Session State (QoS 2 wird beim Wiederverbinden zu Ende geführt, ein
PUBACK ab 0x80 verwirft die Nachricht).

**Tests:** `topic_filter::tests::*`,
`broker::tests::mqtt5_4_8_2_a_shared_subscription_sends_each_message_to_one_session`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_8_2_a_shared_subscription_alternates_and_sends_no_retained_message`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_7_2_a_wildcard_filter_does_not_match_a_topic_beginning_with_dollar`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_2_2_a_topic_name_with_a_wildcard_gets_0x90`;
gegen Mosquitto
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_8_2_two_mosquitto_v5_clients_share_a_subscription`.

**Status:** done — der Test gegen Mosquitto ist `#[ignore]` (Job
`integration-bridges-vendor`), auf dem Linux-Testhost grün.

### §4.9 Flow Control — Server

**Spec:** [MQTT-4.9.0-1] bis [MQTT-4.9.0-3]; §3.3.4 Receive Maximum des
Servers.

**Repo:** `crates/mqtt-bridge/src/broker.rs::Broker::{take_sendable,
resend}` (Send Quota = Receive Maximum des Clients minus die auf dieser
Verbindung unbestätigten Zustellungen; sie beginnt mit jeder Verbindung
neu, auch für die Wiederholung beim Wiederverbinden; QoS 0 und alle
anderen Pakete laufen weiter);
`crates/mqtt-bridge/src/server/conn.rs::Connection::on_publish` (mehr
unbestätigte QoS-2-Nachrichten als das Receive Maximum des Servers: 0x93).

**Tests:** `broker::tests::mqtt5_4_9_the_send_quota_holds_back_qos_1_and_2_not_qos_0`,
`broker::tests::mqtt5_4_9_a_reconnect_resends_within_the_new_receive_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_9_the_server_sends_no_more_than_the_clients_receive_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_9_a_reconnect_resends_no_more_than_the_new_receive_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_4_a_client_over_the_servers_receive_maximum_gets_0x93`.

**Status:** done

### §4.11-§4.12 Server Redirection und Enhanced Authentication — Server

**Spec:** §4.11; [MQTT-4.12.0-1] bis [MQTT-4.12.0-7], [MQTT-4.12.1-1],
[MQTT-4.12.1-2]; einfache Authentifizierung mit User Name und Password
(§5.4.1).

**Repo:** `crates/mqtt-bridge/src/server/mod.rs::{Authenticator,
AuthExchange, AuthStep, Redirect, ServerHandle::redirect_clients}`;
`crates/mqtt-bridge/src/server/conn.rs::Connection::{open, on_auth}` —
AUTH 0x18 bis zum Erfolg, CONNACK und AUTH mit der Authentication Method
des CONNECT, eine unbekannte Methode mit 0x8C, erneute Authentifizierung
mit 0x19, ein Fehlschlag mit DISCONNECT und Schließen, AUTH ohne
Authentication Method im CONNECT als Protocol Error; Umleitung mit
CONNACK oder DISCONNECT 0x9C/0x9D und Server Reference.

**Tests:** `crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_12_enhanced_authentication_and_re_authentication`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_12_simple_authentication_refuses_a_wrong_password`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_11_server_redirection_with_a_server_reference`.

**Status:** done — die Verfahren selbst sind Implementierungen von
`Authenticator`; der Test prüft den Ablauf mit einem Verfahren aus
Challenge und Response, das der Test mitbringt.

### §4.13 Handling Errors — Server

**Spec:** [MQTT-4.13.1-1], [MQTT-4.13.2-1]; [MQTT-1.5.4-1],
[MQTT-1.5.4-2] (UTF-8, U+0000); §3.1.2.11.4 Packet too large.

**Repo:** `crates/mqtt-bridge/src/server/conn.rs::Connection::{run,
close}` — ein Malformed Packet, ein Protocol Error und jeder andere
Fehler mit Reason Code endet mit DISCONNECT und diesem Code (3.1.1: nur
das Schließen), danach Will und Session nach §3.1.2.5; dazu gehören die
Protocol Errors von §3.2.2.3.11 bis §3.2.2.3.13 (0xA2, 0x9E, 0xA1) und
ein Topic Alias 0 mit 0x94;
`crates/mqtt-bridge/src/net.rs::read_packet_within` (Größengrenze vor
dem Lesen des Bodys, Remaining Length über vier Oktette);
`crates/mqtt-bridge/src/data_types.rs::decode_utf8_string` (U+0000).

**Tests:** `net::tests::mqtt5_1_5_5_a_remaining_length_of_five_octets_is_malformed`,
`net::tests::mqtt5_3_1_2_11_4_a_packet_above_the_limit_is_refused_by_its_total_size`,
`data_types::tests::mqtt5_1_5_4_a_string_with_u0000_is_refused`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_13_1_a_malformed_packet_gets_0x81_the_will_and_the_end_of_the_session`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_11_4_a_packet_above_the_servers_maximum_packet_size_gets_0x95`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_features_the_server_does_not_offer`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_2_3_4_topic_aliases_from_the_client`.

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

**Repo:** `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`WsStream`: bietet das Subprotokoll `mqtt` an und weist eine Antwort ohne oder mit anderem Subprotokoll ab; sendet jedes Paket in einem binären Frame; liest die Nutzdaten der Frames mit Opcode binary und continuation als einen Bytestrom; auf einen Frame mit Opcode text folgen ein Close frame mit Statuscode 1003 und das Schließen der Verbindung), `crates/mqtt-bridge/src/daemon/client.rs` (`MqttClient::connect_url`, `ws://`, `wss://`)

**Tests:** `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_1_a_text_frame_from_the_server_closes_the_network_connection`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_2_a_control_packet_split_over_frames_is_read_as_one`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_2_two_control_packets_in_one_frame_are_both_read`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_3_the_client_offers_the_subprotocol_mqtt`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_4_a_server_that_selects_no_or_another_subprotocol_is_refused`; gegen Mosquitto `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_ws_broker_url_carries_mqtt_both_ways_through_mosquitto`, `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate`, `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_a_ws_url_on_a_plain_listener_fails_the_handshake`

**Status:** done — der Client hält [MQTT-6.0.0-1] bis [MQTT-6.0.0-4]; die fünf Tests mit dem Server im selben Prozess laufen im Job `merge-rust-mqtt-bridge`, die drei Tests gegen Mosquitto sind `#[ignore]` und laufen im wöchentlichen Job `integration-bridges-vendor`. Der Server ist das folgende Item.

### §6 MQTT-Server über WebSocket

**Spec:** §6 — [MQTT-6.0.0-1], [MQTT-6.0.0-2] und [MQTT-6.0.0-4] auf der Seite des Servers: binäre Frames, Pakete über Frame-Grenzen hinweg, Subprotokoll `mqtt` auswählen.

**Repo:** `crates/mqtt-bridge/src/server/mod.rs::{MqttBrokerServer::with_websocket, MqttBrokerServer::with_secure_websocket, MqttBrokerServer::with_tls, Transport, ServerHandle::transport_addr, ServerHandle::handshake_failures}` — Listener für WebSocket (`ws`), WebSocket über TLS (`wss`) und TLS (`mqtts`) neben dem TCP-Listener (Features `websocket` und `tls`, beide in `daemon`); die Handshakes laufen im Thread der Verbindung. `crates/mqtt-bridge/src/server/websocket.rs::{accept, WebSocketStream}` — der Opening Handshake nach RFC 6455 §4.2 wählt das Subprotokoll `mqtt` aus; eine Anfrage ohne `mqtt` erhält HTTP 400, eine ungültige den Status ihres Fehlers (426, 405, 400), beide das Schließen; der Pfad der Anfrage wirkt nicht auf MQTT, jeder Pfad erreicht denselben Server. Die Nutzdaten der Frames mit Opcode binary und continuation sind ein Bytestrom, gelesen, während sie ankommen; kein Frame wird ganz gepuffert, die Grenze ist die Maximum Packet Size von MQTT. Ein Frame mit Opcode text schließt die Network Connection mit einem Close frame 1003, erkannt am Header. Ein unmaskierter Frame, ein RSV-Bit, ein reservierter Opcode, ein Verstoß gegen die Fragmentierung und ein ungültiger Close frame beenden die Verbindung mit 1002 oder 1007. Jedes Paket des Servers geht in einem unmaskierten binären Frame hinaus; ein Ping erhält einen Pong, ein Close einen Close mit demselben Statuscode. Beim Schließen durch den Server folgen Close 1000 und das Ende seiner Richtung; die TCP-Verbindung schließt, wenn der Close frame des Clients da ist, sonst nach zwei Sekunden (`crates/mqtt-bridge/src/server/websocket.rs::CLOSE_WAIT`). `crates/mqtt-bridge/src/server/tls.rs::TlsStream` — TLS mit dem Handshake der Bridges und der `rustls::ServerConfig` des Aufrufers. `crates/mqtt-bridge/src/ws.rs` — das Subprotokoll, das Lesen des Kopfs des Opening Handshake und der Grund im Close frame, gemeinsam mit dem Client.

**Tests:** `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_4_the_server_selects_the_subprotocol_mqtt`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_4_a_client_that_does_not_offer_mqtt_is_refused`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_request_path_has_no_impact_on_mqtt`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_an_invalid_opening_handshake_gets_an_http_error_response`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_opening_handshake_has_a_deadline`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_1_a_text_frame_closes_the_network_connection`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_1_the_server_sends_each_control_packet_in_one_unmasked_binary_frame`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_2_a_control_packet_split_over_frames_is_read_as_one`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_2_several_control_packets_in_one_frame_are_all_handled`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_invalid_frames_of_the_client_fail_the_connection`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_a_close_frame_of_the_client_is_answered_and_its_will_published`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_server_ends_a_connection_with_disconnect_then_a_close_frame`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_without_a_close_frame_of_the_client_the_server_closes_after_close_wait`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_server_answers_a_ping_with_a_pong_of_the_same_data`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_a_message_crosses_between_the_tcp_and_the_websocket_listener`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_zerodds_client_publishes_and_subscribes_over_wss_and_mqtts`, `crates/mqtt-bridge/tests/contract_websocket_bridge.rs::ws_4_2_1_a_request_is_read_with_its_offered_subprotocols_and_refused_with_its_status`, `crates/mqtt-bridge/tests/contract_websocket_bridge.rs::ws_4_2_2_the_response_carries_the_accept_value_and_only_the_selected_subprotocol`, `crates/mqtt-bridge/tests/contract_websocket_bridge.rs::ws_5_2_the_header_of_a_frame_is_read_before_its_payload`, `crates/mqtt-bridge/tests/contract_websocket_bridge.rs::ws_7_4_1_the_status_codes_and_the_close_body`, `crates/mqtt-bridge/tests/contract_bridge_security.rs::mqtt5_4_2_serve_tls_handshake_finishes_the_handshake_or_fails_within_its_timeout`; gegen Eclipse Paho (Python-Client `paho-mqtt`, `transport="websockets"`) `crates/mqtt-bridge/tests/paho_websocket_server.rs::mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_websocket`, `crates/mqtt-bridge/tests/paho_websocket_server.rs::mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_wss`.

**Status:** done — die 16 Tests in `websocket_server.rs`, die vier Tests des Vertrags mit `zerodds-websocket-bridge` und der Test des Vertrags mit `zerodds-bridge-security` laufen im Job `merge-rust-mqtt-bridge`; die zwei Tests gegen Paho sind `#[ignore]` und laufen im wöchentlichen Job `integration-bridges-vendor` (Eintrag `paho` in `tests/integration/provision.sh`). Auf dem Linux-Testhost grün, gegen Paho 2.1.0 (Debian-Paket `python3-paho-mqtt`) in MQTT 5.0 und 3.1.1 über `ws` und `wss`.

## §7 Conformance

### §7 Conformance Targets

**Spec:** §7 — Drei Conformance-Targets: MQTT-Client, MQTT-Server,
MQTT-Server-with-MQTT-Bridge-Functionality.

**Repo:** Codec, Logik des Brokers und Server im Netz im Crate `mqtt-bridge`.
Die Targets *MQTT-Client* (`net.rs::MqttClient`) und *MQTT-Server*
(`crates/mqtt-bridge/src/server/mod.rs::MqttBrokerServer`) sind realisiert und gegen die Eclipse-
Mosquitto-Referenz interop-getestet (`tests/mosquitto_interop_e2e.rs`), der Server über WebSocket
zusätzlich gegen Eclipse Paho (`tests/paho_websocket_server.rs`).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs`, `crates/mqtt-bridge/tests/paho_websocket_server.rs::mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_websocket`, `crates/mqtt-bridge/tests/paho_websocket_server.rs::mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_wss`.

**Status:** n/a (informative) — benennt die Konformitätsziele Client, Server und Server mit Bridge-Funktion; die Anforderungen sind die Items dieses Dokuments.

---

## Audit-Status

37 done / 0 partial / 0 open / 0 blocked / 3 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document mqtt-5.0.md`):

* `cargo nextest run --profile merge -p zerodds-mqtt-bridge --all-targets` (Job `merge-rust-mqtt-bridge`) — 45 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor` (Job `integration-bridges-vendor`) — 7 Tests grün.

52 zitierte Tests grün, 0 nicht grün.

