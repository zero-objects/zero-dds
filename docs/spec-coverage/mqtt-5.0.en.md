# OASIS MQTT v5.0 — Spec Coverage

**Spec:** [OASIS MQTT Version 5.0 — Standard, 07 March 2019 →](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html)

**Context:** MQTT 5.0 is the OASIS pub-sub spec for IoT/M2M. ZeroDDS
implements the wire codec (spec §1.5 Data Types, §2.1 Fixed Header, §2.2
Variable Header + Properties, chapter 3) as a pure-Rust no_std+alloc
library, the session engine of a server (chapter 4) and a server on the
network that holds the server statements of chapters 3, 4 and 6.

Implementation:

- `crates/mqtt-bridge/` — wire codec, session engine (`broker.rs`),
  server (`server/`), client (`net.rs`, `daemon/client.rs`), DDS bridge.

---

## §1 Introduction

### §1.1-§1.4 Background

**Spec:** §1, p. 7-10 (spec) — abstract, status, status of this doc.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — editorial background; without a code
mapping.

### §1.5 Data types

**Spec:** §1.5 — all MQTT data types:
* §1.5.1 Bits — implicit via `u8`.
* §1.5.2 Two Byte Integer (BE u16).
* §1.5.3 Four Byte Integer (BE u32) — implicit via `u32::to_be_bytes`.
* §1.5.4 UTF-8 Encoded String (u16 BE length + UTF-8).
* §1.5.5 Variable Byte Integer (1-4 bytes, 7-bit groups + continuation
  bit, max 268,435,455).
* §1.5.6 Binary Data (u16 BE length + bytes).
* §1.5.7 UTF-8 String Pair — composed of 2 §1.5.4 strings (the caller
  builds it via 2 `encode_utf8_string` calls).

**Repo:** `crates/mqtt-bridge/src/data_types.rs` +
`crates/mqtt-bridge/src/vbi.rs`.

**Tests:** `data_types::tests::mqtt5_1_5_2_a_two_byte_integer_is_big_endian`,
`mqtt5_1_5_4_a_string_is_its_length_and_its_utf8_octets`, `mqtt5_1_5_utf8_string_starts_with_be_length`,
`mqtt5_1_5_utf8_string_decode_invalid_utf8`, `mqtt5_1_5_utf8_string_decode_truncated`,
`mqtt5_1_5_6_binary_data_is_its_length_and_its_octets`; `vbi::tests::*` (10 tests, all boundaries
0/127/128/16383/16384/2097151/2097152/MAX, 5-byte reject).

**Status:** done

---

## §2 MQTT control packet format

### §2.1 Structure of an MQTT control packet

**Spec:** §2.1, p. 19-21 — Fixed Header (type nibble + flags nibble +
Remaining Length VBI), Variable Header, Payload.

**Repo:** `crates/mqtt-bridge/src/packet.rs::FixedHeader`.

**Tests:** `packet::tests::mqtt5_2_1_publish_header_decodes_dup_qos_retain_flags`,
`codec::tests::mqtt5_2_1_fixed_header_first_byte_layout_for_publish`.

**Status:** done

### §2.1.2 Control packet type (Table 2-1)

**Spec:** §2.1.2 — 15 packet types (CONNECT=1, ..., AUTH=15).

**Repo:** `crates/mqtt-bridge/src/packet.rs::ControlPacketType` with all 15
values.

**Tests:** `packet::tests::mqtt5_2_1_2_the_packet_types_are_the_values_of_table_2_1`,
`mqtt5_2_1_2_packet_type_zero_is_reserved`.

**Status:** done

### §2.1.4 Remaining Length

**Spec:** §2.1.4 — VBI; bytes after the Fixed Header to the end of the
packet.

**Repo:** cross-ref the VBI module.

**Tests:** `codec::tests::mqtt5_2_1_4_large_payload_encodes_multibyte_remaining_length`,
`mqtt5_2_1_4_truncated_remaining_length_decode_fails`.

**Status:** done

### §2.2 Variable Header

**Spec:** §2.2, p. 21 — per-packet-type variable-header content.

**Repo:** the PUBLISH variable header in
`crates/mqtt-bridge/src/codec.rs::encode_publish/decode_publish`.

**Tests:** `codec::tests::mqtt5_3_3_2_2_a_packet_identifier_only_above_qos_0`,
`codec::tests::mqtt5_3_3_2_the_publish_of_figure_3_9`.

**Status:** done — for PUBLISH; further packet types implementable
analogously.

### §2.2.2 Properties

**Spec:** §2.2.2, p. 23-25 — a VBI-length-prefixed property block; property
identifier (spec Table 2-4) + value.

**Repo:** `crates/mqtt-bridge/src/properties.rs::{PropertyId, Property, PropertyValueKind}`:
one variant per identifier of Table 2-4 (27), `PropertyId::value_kind()` returns
the data type of the table, `None` for an identifier outside the table
(`Other(u32)`). `crates/mqtt-bridge/src/control_packets.rs::property_data_type`
reads the same table; `property_value` refuses a property block with an unknown
identifier, because its value cannot be delimited (§2.2.2.2). The codec
serializes the property block as bytes with a VBI length prefix.
`crates/mqtt-bridge/src/property_set.rs::{PropertySet, allowed_in}` reads a
block typed and checks it against the packet column of Table 2-4: an
identifier the table does not list for the packet is a Malformed Packet, a
property other than User Property and Subscription Identifier twice and a
forbidden value are a Protocol Error, a Topic Alias of 0 with the more
specific Reason Code 0x94 (§3.3.2.3.4); `PropertySet::encode` writes the
table order, User Properties in their order. The server
(`crates/mqtt-bridge/src/server/`) reads and writes every property block
through it.

**Tests:** `properties::tests::mqtt5_2_2_2_all_27_identifiers_of_table_2_4_and_their_types`,
`properties::tests::mqtt5_2_2_2_an_identifier_outside_table_2_4_has_no_value_type`,
`codec::tests::mqtt5_3_3_2_3_the_properties_follow_their_length`,
`property_set::tests::mqtt5_2_2_2_a_set_round_trips_and_keeps_the_order_of_user_properties`,
`property_set::tests::mqtt5_2_2_2_an_identifier_not_listed_for_the_packet_is_malformed`,
`property_set::tests::mqtt5_2_2_2_a_property_twice_is_a_protocol_error_user_property_is_not`,
`property_set::tests::mqtt5_2_2_2_forbidden_values_are_protocol_errors`,
`property_set::tests::mqtt5_1_5_4_a_string_with_u0000_or_bad_utf8_and_a_cut_value_are_malformed`.

**Status:** done — all 27 identifiers of Table 2-4 with value and data type; an
identifier outside the table has no data type.

---

## §3 MQTT control packets

### §3.1 CONNECT

**Spec:** §3.1, p. 30-46 — connection setup with Will/properties/authentication.

**Repo:** `crates/mqtt-bridge/src/control_packets.rs::{encode_connect_body, decode_connect_body}` (`ConnectBody`).

**Tests:** `control_packets::tests::mqtt5_3_1_3_without_will_and_credentials_the_payload_is_the_client_identifier`,
`control_packets::tests::mqtt5_3_1_2_12_the_connect_of_figure_3_6_and_its_payload_in_the_order_of_3_1_3`.

**Status:** done

### §3.2 CONNACK

**Spec:** §3.2 — Connect Acknowledgment.

**Repo:** `control_packets::{encode_connack_body, decode_connack_body}` (`ConnackBody`).

**Tests:** `control_packets::tests::mqtt5_3_2_2_a_connack_is_flags_reason_code_and_properties`.

**Status:** done

### §3.3 PUBLISH — fully covered

**Spec:** §3.3, p. 48-58 — the complete PUBLISH packet spec with
DUP/QoS/RETAIN flags, topic name, packet identifier (at QoS>0), properties,
payload.

**Repo:** `crates/mqtt-bridge/src/codec.rs::encode_publish/decode_publish`
fully implemented; QoS-3 is rejected (MalformedPacket), the
packet-identifier requirement at QoS>0 is enforced.

**Tests:** `codec::tests::mqtt5_3_3_2_2_a_packet_identifier_only_above_qos_0`,
`mqtt5_3_3_2_the_publish_of_figure_3_9`,
`mqtt5_3_3_invalid_qos_3_rejected_on_encode`, `mqtt5_3_3_missing_packet_id_at_qos1_rejected`,
`mqtt5_3_3_wrong_packet_type_rejected_on_decode`,
`mqtt5_2_1_fixed_header_first_byte_layout_for_publish`,
`mqtt5_3_3_2_3_the_properties_follow_their_length`,
`mqtt5_2_1_4_truncated_remaining_length_decode_fails`, `mqtt5_3_3_empty_input_decode_fails`,
`mqtt5_2_1_4_large_payload_encodes_multibyte_remaining_length`.

**Status:** done

### §3.4 PUBACK

**Spec:** §3.4 — acknowledgment for a QoS-1 PUBLISH.

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}` (shared
`AckBody` structure).

**Tests:** `control_packets::tests::mqtt5_3_4_2_a_puback_is_packet_id_reason_code_and_properties`.

**Status:** done

### §3.5 PUBREC

**Spec:** §3.5 — QoS-2 receive acknowledgment (step 1 of the 4-way
handshake).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}` (same
AckBody structure).

**Tests:** `control_packets::tests::mqtt5_3_4_2_a_puback_is_packet_id_reason_code_and_properties`.

**Status:** done

### §3.6 PUBREL

**Spec:** §3.6 — QoS-2 publish release (step 2).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`.

**Tests:** `control_packets::tests::mqtt5_3_4_2_a_puback_is_packet_id_reason_code_and_properties`.

**Status:** done

### §3.7 PUBCOMP

**Spec:** §3.7 — QoS-2 publish complete (step 3, the final ACK).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`.

**Tests:** `control_packets::tests::mqtt5_3_4_2_a_puback_is_packet_id_reason_code_and_properties`.

**Status:** done

### §3.8 SUBSCRIBE

**Spec:** §3.8 — client-to-server subscription request with a topic-filter
list.

**Repo:** `control_packets::{encode_subscribe_body, decode_subscribe_body}`.

**Tests:** `control_packets::tests::mqtt5_3_8_the_subscribe_of_figures_3_19_and_3_21`.

**Status:** done

### §3.9 SUBACK

**Spec:** §3.9 — server-to-client SUBSCRIBE acknowledgment with reason codes
per subscription.

**Repo:** `control_packets::{encode_suback_body, decode_suback_body}`.

**Tests:** `control_packets::tests::mqtt5_3_9_a_suback_is_packet_id_properties_and_a_reason_code_per_filter`.

**Status:** done

### §3.10 UNSUBSCRIBE

**Spec:** §3.10 — client unsubscribe request.

**Repo:** `control_packets::{encode_unsubscribe_body, decode_unsubscribe_body}`.

**Tests:** `control_packets::tests::mqtt5_3_10_the_unsubscribe_payload_of_figure_3_30`.

**Status:** done

### §3.11 UNSUBACK

**Spec:** §3.11 — server UNSUBSCRIBE acknowledgment with reason codes.

**Repo:** `control_packets::{encode_unsuback_body, decode_unsuback_body}`.

**Tests:** `control_packets::tests::mqtt5_3_11_an_unsuback_is_packet_id_properties_and_a_reason_code_per_filter`.

**Status:** done

### §3.12 PINGREQ

**Spec:** §3.12 — a 2-byte heartbeat (type byte + Remaining=0).

**Repo:** constant `0xC0`; encode trivial (1 byte 0xC0 + VBI(0)).

**Tests:** `packet::tests::mqtt5_2_1_2_the_packet_types_are_the_values_of_table_2_1`.

**Status:** done

### §3.13 PINGRESP

**Spec:** §3.13 — a 2-byte heartbeat reply (type byte + Remaining=0).

**Repo:** constant `0xD0`; encode trivial (1 byte 0xD0 + VBI(0)).

**Tests:** `packet::tests::mqtt5_2_1_2_the_packet_types_are_the_values_of_table_2_1`.

**Status:** done

### §3.14 DISCONNECT

**Spec:** §3.14 — connection teardown (with a reason code, new in 5.0).

**Repo:** `control_packets::{encode_disconnect_body, decode_disconnect_body}`.

**Tests:** `control_packets::tests::mqtt5_3_14_2_the_disconnect_of_figure_3_24`.

**Status:** done

### §3.15 AUTH

**Spec:** §3.15 — extended authentication (new in 5.0, multi-step
SASL-like challenge/response).

**Repo:** `control_packets::{encode_auth_body, decode_auth_body}`.

**Tests:** `control_packets::tests::mqtt5_3_15_2_an_auth_is_reason_code_and_properties`.

**Status:** done

---

## §4 Operational behavior

### §4.1-§4.13 Storing state + network connections + QoS levels

**Spec:** §4 — session state, topic-filter matching, the QoS flows,
Keep Alive, message ordering.

**Repo:** `crates/mqtt-bridge/src/broker.rs::{Broker, Session,
Subscription, RetainedMessage, DeliveryEnvelope, AppMessage,
MessageProperties, QoS, Will}` — the session engine without a network; time is
the caller's milliseconds. It holds the Session Expiry Interval and
Session Present, the messages pending transmission (QoS 0 only while the
connection is open; no limit by default, with a limit per session a full
queue does not keep QoS 0 and a QoS 1 or QoS 2 message reports the
session for its end, `Delivered::over_limit`, `Broker::end_session`),
the QoS 1 and QoS 2 deliveries in the order they were sent, the
received QoS 2 Packet Identifiers until the PUBREL, the Will with its Will
Delay Interval and Will Properties. Routing: one copy per session with
the highest QoS and every Subscription Identifier, No Local, Retain As
Published, Retain Handling, Shared Subscriptions, Message Expiry, the
send quota (§4.9), retransmission only at reconnect within the new
connection's Receive Maximum (§4.4). Topic-filter
matching in `crates/mqtt-bridge/src/topic_filter.rs`, Keep Alive in
`crates/mqtt-bridge/src/keep_alive.rs::KeepAliveTracker` (1.5 × Keep
Alive). The network around it is described by the following items.

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

**Status:** done — the engine carries the session state of §4.1 and the
routing; what the server does with it on the network is shown by the
items from "§4 Standalone broker server" on.

### §4 Standalone broker server (network exposure of the broker logic)

**Spec:** §4 + §7.1.1 (conformance target *MQTT Server*) — a listening
server accepts client connections and holds the server statements of
chapters 3 and 4. The statements are grouped in the items "— server"
below.

**Repo:** `crates/mqtt-bridge/src/server/mod.rs::{MqttBrokerServer,
ServerHandle, ServerConfig, ClientStream, Authenticator}` — a
`TcpListener` with an accept loop, beside it the listeners for WebSocket,
TLS and WebSocket over TLS of the item "§6 MQTT server over WebSocket";
per connection a reader thread (the
protocol, `crates/mqtt-bridge/src/server/conn.rs`) and a writer thread as
the socket's only writer. The session state lives in the engine
`crates/mqtt-bridge/src/broker.rs` behind one lock; a housekeeping thread
publishes delayed Wills, ends expired sessions and deletes expired
messages. `ServerConfig` holds what the server decides itself: Receive
Maximum, Maximum Packet Size, Topic Alias Maximum, Maximum QoS, Retain
Available, Wildcard Subscription Available, Subscription Identifiers
Available, Shared Subscription Available, Server Keep Alive, the longest
Session Expiry Interval, Response Information, redirection, the limit of
messages per session (`max_queued_messages`, no limit by default) and the
`Authenticator`. The protocol
version comes per client from the Protocol Level, 5.0 and 3.1.1 at the
same server. `ClientStream` accepts connections of other transports
(`ServerHandle::serve`). `crates/mqtt-bridge/src/server/inbound.rs` reads
every packet of a client strictly and returns the Reason Code of §4.13 on
an error. `crates/mqtt-bridge/src/net.rs::{MqttClient,
read_packet_within}` — TCP framing with a size limit and a lightweight
client.

**Tests:** `crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_publish_subscribe_qos1`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_retained_message_delivered_to_late_subscriber`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_wildcard_plus_and_hash`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_qos2_exactly_once_delivery`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_cross_version_v5_publishes_v311_subscribes`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_cross_version_v311_publishes_v5_subscribes`,
`crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_pingreq_is_answered_with_a_bodyless_pingresp_within_the_keep_alive`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_2_a_connection_of_another_transport_is_served_through_client_stream`;
against Mosquitto
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_v5_clients_through_zerodds_broker`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_v311_clients_through_zerodds_broker`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_zerodds_client_through_zerodds_broker_cross_version`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_zerodds_311_client_receives_from_mosquitto`,
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_311_client_receives_from_zerodds`.

**Status:** done — the tests against Mosquitto are `#[ignore]` and run in
the weekly job `integration-bridges-vendor`; green on the Linux test host
against Mosquitto 2.0.21.

### §3.1 CONNECT — server

**Spec:** [MQTT-3.1.0-1], [MQTT-3.1.0-2], [MQTT-3.1.2-1] to
[MQTT-3.1.2-15], [MQTT-3.1.2-22], [MQTT-3.1.2-23], [MQTT-3.1.2-28],
[MQTT-3.1.3-5] to [MQTT-3.1.3-10], [MQTT-3.1.4-1] to [MQTT-3.1.4-6].

**Repo:** `crates/mqtt-bridge/src/server/inbound.rs::parse_connect`
(Protocol Name and Level, the reserved flag, the Will flags, the User
Name and Password flags, the property blocks, the order of the fields, no
octet after the last field; 0x84 in the format the client can read);
`crates/mqtt-bridge/src/server/conn.rs::Connection::open` (the first
packet is CONNECT, Assigned Client Identifier, takeover with DISCONNECT
0x8E, Clean Start, the Will with Will Delay Interval and Will Properties,
Keep Alive through `KeepAliveTracker`, a Will QoS above Maximum QoS with
0x9B, a Will Retain without Retain Available with 0x9A),
`Connection::handle` (a second CONNECT is a Protocol Error),
`Connection::own_session` (after the takeover or the end of the session
with 0x97 the old connection handles no packet against the session any
more: what it still reads ends it).

**Tests:** `server::inbound::tests::mqtt5_3_1_2_the_protocol_name_level_and_reserved_flag_are_checked`,
`server::inbound::tests::mqtt5_3_1_2_5_will_flags_without_a_will_and_trailing_octets_are_malformed`,
`server::inbound::tests::mqtt5_3_1_3_the_3_1_1_return_codes_of_a_refusal`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_0_2_a_second_connect_is_a_protocol_error`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_2_an_unsupported_protocol_version_gets_0x84_and_the_close`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_3_the_reserved_connect_flag_gets_0x81`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_3_1_an_empty_client_id_gets_an_assigned_client_identifier`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_4_3_a_second_connection_takes_the_session_over_with_0x8e`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_4_3_what_the_replaced_connection_still_reads_does_not_reach_the_session`
(the old connection sends a SUBSCRIBE and a DISCONNECT with Session Expiry
Interval 0 after its DISCONNECT 0x8E, over a stream the server reads on
after its close: the session of the new connection gets no subscription
and keeps its Session Expiry Interval),
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_10_keep_alive_closes_after_one_and_a_half_periods_and_publishes_the_will`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_3_2_2_a_delayed_will_waits_and_a_reconnect_cancels_it`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_2_28_response_information_only_on_request`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_0_1_session_present_and_clean_session_of_3_1_1_clients`.

**Status:** done — green on the Linux test host. Before, the old
connection handled the packets its reader still held after the takeover
against the session of the new one: without the check in `own_session`
the new connection receives the message on `old/t` in the test (measured
with a mutation).

### §3.2 CONNACK — server

**Spec:** [MQTT-3.2.0-1], [MQTT-3.2.0-2], [MQTT-3.2.2-1] to
[MQTT-3.2.2-3], [MQTT-3.2.2-6] to [MQTT-3.2.2-10], [MQTT-3.2.2-12],
[MQTT-3.2.2-13], [MQTT-3.2.2-16], [MQTT-3.2.2-19] to [MQTT-3.2.2-22].

**Repo:** `crates/mqtt-bridge/src/server/conn.rs::Connection::open` — the
CONNACK goes out before any other packet except AUTH, once per
connection; Session Present by §3.2.2.1.1, 0 with a non-zero Reason Code;
the properties from `ServerConfig` (Session Expiry Interval when the
server takes another value; Receive Maximum; Maximum QoS; Retain
Available; Maximum Packet Size; Assigned Client Identifier; Topic Alias
Maximum; Wildcard Subscription Available, Subscription Identifier
Available and Shared Subscription Available; Server Keep Alive; Response
Information; Authentication Method and Data). From Reason Code 0x80 on
the connection is closed. Server Keep Alive replaces only the Keep Alive
of a 5.0 client, which gets it in the CONNACK; a 3.1.1 client keeps its
own ([MQTT-3.2.2-22]). What the CONNACK announces as not available ends
the connection of a 5.0 client: a Wildcard Subscription with DISCONNECT
0xA2, a Shared Subscription with 0x9E, a Subscription Identifier with
0xA1 (§3.2.2.3.11 to §3.2.2.3.13). `crates/mqtt-bridge/src/server/inbound.rs::refusal` maps the
Reason Codes to the Return Codes of 3.1.1.

**Tests:** `crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_connack_carries_the_server_limits_and_session_present_follows_clean_start`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_features_the_server_does_not_offer`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_14_the_server_keep_alive_replaces_the_clients`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_22_a_3_1_1_client_keeps_its_own_keep_alive_beside_a_server_keep_alive`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_2_a_session_expiry_above_the_servers_maximum_is_answered`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_1_3_1_an_empty_client_id_gets_an_assigned_client_identifier`.

**Status:** done — the server sends no Reason String and no User Property
in the CONNACK; [MQTT-3.2.2-19] and [MQTT-3.2.2-20] hold without a test
of their own.

### §3.3 PUBLISH — server

**Spec:** [MQTT-3.3.1-1] to [MQTT-3.3.1-13], [MQTT-3.3.2-1] to
[MQTT-3.3.2-8], [MQTT-3.3.2-11] to [MQTT-3.3.2-20], [MQTT-3.3.4-1] to
[MQTT-3.3.4-6], [MQTT-3.3.4-9], [MQTT-3.3.4-10], [MQTT-3.1.2-24] to
[MQTT-3.1.2-27]; §3.3.2.3.2 Payload Format Indicator.

**Repo:** `crates/mqtt-bridge/src/server/inbound.rs::parse_publish`
(QoS 3, DUP at QoS 0, Packet Identifier 0); `crates/mqtt-bridge/src/server/conn.rs::Connection::on_publish`
(Topic Name without a wildcard, the client's Topic Alias up to the
server's Topic Alias Maximum, a Topic Alias of 0 or above it with 0x94 —
`crates/mqtt-bridge/src/property_set.rs::PropertySet::parse` checks 0 —,
no Subscription Identifier from the client,
the server's Receive Maximum, Payload Format Indicator with 0x99, PUBACK
and PUBREC with 0x10 without a matching subscriber), `publish_bytes`
(properties unaltered, the Message Expiry Interval as what is left, the
Subscription Identifiers, a Topic Alias to the client up to its maximum,
DUP only on a retransmission, a message over the client's Maximum Packet
Size is discarded and counts as sent), `pump` (the client's Receive
Maximum).

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
against Mosquitto
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_3_3_2_3_mosquitto_v5_properties_cross_the_zerodds_broker_unaltered`.

**Status:** done — the test against Mosquitto is `#[ignore]` and runs in
the weekly job `integration-bridges-vendor`; green on the Linux test host
against Mosquitto 2.0.21.

### §3.4-§3.7 PUBACK, PUBREC, PUBREL, PUBCOMP — server

**Spec:** [MQTT-3.4.2-1] to [MQTT-3.4.2-3], [MQTT-3.5.2-1] to
[MQTT-3.5.2-3], [MQTT-3.6.1-1], [MQTT-3.6.2-1] to [MQTT-3.6.2-3],
[MQTT-3.7.2-1] to [MQTT-3.7.2-3].

**Repo:** `crates/mqtt-bridge/src/server/inbound.rs::parse_ack`
(flags, PUBREL with 0b0010, the short forms, the Reason Codes of the
packet's table); `crates/mqtt-bridge/src/server/conn.rs::Connection::send_ack`
and `send_fitted` (a Reason String only with an error, without the
properties when they would exceed the client's Maximum Packet Size);
PUBREL and PUBCOMP with 0x92 for an unknown Packet Identifier.

**Tests:** `server::inbound::tests::mqtt5_3_4_2_acks_short_forms_flags_and_reason_codes`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_6_1_a_pubrel_with_flags_0_is_malformed`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_4_2_1_a_puback_says_0x10_without_matching_subscribers`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_6_2_2_a_reason_string_beyond_the_clients_maximum_packet_size_is_left_out`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_3_3_a_duplicate_qos2_publish_is_delivered_once`.

**Status:** done

### §3.8-§3.11 SUBSCRIBE, SUBACK, UNSUBSCRIBE, UNSUBACK — server

**Spec:** [MQTT-3.8.1-1], [MQTT-3.8.3-1] to [MQTT-3.8.3-5],
[MQTT-3.8.4-1] to [MQTT-3.8.4-8], [MQTT-3.9.2-1], [MQTT-3.9.2-2],
[MQTT-3.9.3-1], [MQTT-3.9.3-2], [MQTT-3.10.1-1], [MQTT-3.10.3-1],
[MQTT-3.10.3-2], [MQTT-3.10.4-1] to [MQTT-3.10.4-6], [MQTT-3.11.2-1],
[MQTT-3.11.2-2], [MQTT-3.11.3-1], [MQTT-3.11.3-2].

**Repo:** `crates/mqtt-bridge/src/server/inbound.rs::{parse_subscribe,
parse_unsubscribe}` (flags 0b0010, at least one entry, reserved bits,
Maximum QoS and Retain Handling 3); `crates/mqtt-bridge/src/server/conn.rs::Connection::{on_subscribe,
on_unsubscribe}` (each entry as a SUBSCRIBE of its own, a Reason Code per
Topic Filter in their order, 0x8F for an invalid Topic Filter, No Local
on a Shared Subscription as a Protocol Error, the granted QoS up to
Maximum QoS, the SUBACK before the retained messages, UNSUBACK with 0x00
or 0x11). A Wildcard Subscription or Shared Subscription the CONNACK
announced as not available, and a Subscription Identifier without
Subscription Identifiers Available, are a Protocol Error for a 5.0
client: DISCONNECT 0xA2, 0x9E or 0xA1 and the close ([MQTT-4.13.1-1]); a
3.1.1 client gets 0x80 for the entry.

**Tests:** `server::inbound::tests::mqtt5_3_8_3_subscribe_flags_options_and_payload_are_checked`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_9_3_suback_and_unsuback_reason_codes_and_request_problem_information`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_8_3_5_reserved_subscription_option_bits_and_a_shared_no_local`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_8_3_3_no_local_keeps_a_message_from_its_publisher`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_1_3_retain_handling_and_retain_as_published`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_features_the_server_does_not_offer`.

**Status:** done

### §3.12-§3.15 PINGREQ, DISCONNECT, AUTH — server

**Spec:** [MQTT-3.12.4-1], [MQTT-3.14.0-1], [MQTT-3.14.1-1],
[MQTT-3.14.2-1], [MQTT-3.14.2-2], [MQTT-3.14.4-1] to [MQTT-3.14.4-3],
[MQTT-3.15.1-1], [MQTT-3.15.2-1]; §3.14.2.2.2 Session Expiry Interval in
the DISCONNECT.

**Repo:** `crates/mqtt-bridge/src/server/conn.rs::Connection::handle`
(PINGRESP; the client's DISCONNECT: 0x00 discards the Will, every other
code publishes it, a non-zero Session Expiry Interval is a Protocol Error
when the CONNECT asked for 0 — checked against the CONNECT value, not the
granted one), `send_disconnect` (the server's DISCONNECT only after the
CONNACK, never with a Session Expiry Interval; DISCONNECT and close go to
the writer thread as one message `Outbound::Disconnect`, and what is
queued after it is not written), `ServerHandle::shutdown` (0x8B; a
connection whose CONNECT comes only after it gets CONNACK 0x88 Server
unavailable in `Connection::open`, under 3.1.1 the Return Code 0x03, and is
closed: 0x8B is no CONNACK Reason Code, [MQTT-3.2.2-8]);
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

**Status:** done — green on the Linux test host. Before, a connection
accepted before the shutdown that sent its CONNECT after it got CONNACK
0x00 and stayed open without end (read in the code; the test above fails
with the old code, measured with a mutation).

### §4.1 Session state — server

**Spec:** [MQTT-4.1.0-1], [MQTT-4.1.0-2], [MQTT-3.1.2-23] — the server
keeps the session beyond the end of the connection while the Session
Expiry Interval runs, and discards it afterwards.

**Repo:** `crates/mqtt-bridge/src/broker.rs::Broker::{connect_session,
disconnect_session, tick, next_deadline, deliver, end_session}`; the
thread `housekeeping` in `crates/mqtt-bridge/src/server/mod.rs`;
`crates/mqtt-bridge/src/server/conn.rs::route`;
`ServerConfig::{maximum_session_expiry, max_queued_messages}`.

**Tests:** `broker::tests::mqtt5_4_1_a_session_ends_when_its_expiry_interval_has_passed`,
`broker::tests::mqtt5_4_1_qos0_is_not_kept_for_an_offline_session_and_a_full_queue_ends_the_session`,
`broker::tests::mqtt5_4_1_at_a_full_queue_qos0_is_not_kept_and_the_ended_session_publishes_its_will`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_1_a_session_ends_after_its_expiry_interval`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_1_a_full_queue_ends_the_session_with_0x97_and_drops_no_message_from_it`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_2_2_3_2_a_session_expiry_above_the_servers_maximum_is_answered`;
against Mosquitto
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_3_1_2_5_a_mosquitto_v5_session_gets_its_queued_message_at_reconnect`.

**Status:** done — the server keeps no QoS 0 messages for a session
without a connection (§4.1 allows it, it does not require it).
`ServerConfig::max_queued_messages` has no limit by default. With a
limit, the server does not keep a QoS 0 message at a full queue; a QoS 1
or QoS 2 message ends the session: DISCONNECT 0x97 Quota exceeded to a
5.0 client, the close of the connection, then the server discards the
session state and publishes the Will (§4.1.1, [MQTT-3.1.2-8]). A QoS 1
or QoS 2 message is thus never missing from a session that goes on. Both
count in `ServerHandle::queue_overflows`. The test against Mosquitto is
`#[ignore]` (job `integration-bridges-vendor`), green on the Linux test
host.

### §4.3-§4.6 QoS flows, retransmission, message receipt, ordering — server

**Spec:** [MQTT-4.3.1-1], [MQTT-4.3.2-1] to [MQTT-4.3.2-5],
[MQTT-4.3.3-1] to [MQTT-4.3.3-13], [MQTT-4.4.0-1], [MQTT-4.4.0-2],
[MQTT-4.5.0-1], [MQTT-4.6.0-5], [MQTT-4.6.0-6].

**Repo:** `crates/mqtt-bridge/src/broker.rs::Broker::{take_sendable,
puback, pubrec, pubcomp, resend, qos2_received, qos2_released}` — an
unused Packet Identifier per delivery, PUBREL after PUBREC, no PUBLISH
after the PUBREL, PUBACK and PUBREC from 0x80 on count as the
acknowledgement; incoming QoS 2 by method B (onward delivery at the
PUBLISH, the Packet Identifier kept until the PUBREL, a duplicate is not
delivered again); retransmission only at reconnect, in the original
order, with DUP and the original Packet Identifier, the PUBLISH packets
within the new connection's Receive Maximum, the others as PUBACK or
PUBCOMP free the send quota (`InFlight::resend_pending`); one queue per
session keeps the order per topic. A message goes into the session state
of every matching client ([MQTT-4.5.0-1]); at a full queue the session
ends, see §4.1.

**Tests:** `broker::tests::mqtt5_4_4_resend_is_in_order_and_a_failed_pubrec_discards`,
`broker::tests::mqtt5_4_3_3_a_duplicate_qos2_publish_is_not_a_new_message_until_pubrel`,
`broker::tests::mqtt5_4_3_2_a_packet_identifier_in_flight_is_not_assigned_again`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_4_unacknowledged_messages_are_sent_again_at_reconnect_with_dup`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_9_a_reconnect_resends_no_more_than_the_new_receive_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_1_a_full_queue_ends_the_session_with_0x97_and_drops_no_message_from_it`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_3_3_a_duplicate_qos2_publish_is_delivered_once`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_6_messages_of_a_topic_arrive_in_the_order_they_were_published`.

**Status:** done

### §4.7-§4.8 Topic names, topic filters, shared subscriptions — server

**Spec:** [MQTT-4.7.0-1], [MQTT-4.7.1-1], [MQTT-4.7.1-2],
[MQTT-4.7.2-1], [MQTT-4.7.3-1] to [MQTT-4.7.3-4], [MQTT-4.8.2-1] to
[MQTT-4.8.2-6].

**Repo:** `crates/mqtt-bridge/src/topic_filter.rs`;
`crates/mqtt-bridge/src/broker.rs::{parse_shared_filter, Broker::subscribe_with}`
(`$share/{ShareName}/{filter}`, no retained messages for a Shared
Subscription, per message one session of the group, a connected one
first, in turn); the delivery to the chosen session runs through its
session state (QoS 2 is completed at reconnect, a PUBACK from 0x80 on
discards the message).

**Tests:** `topic_filter::tests::*`,
`broker::tests::mqtt5_4_8_2_a_shared_subscription_sends_each_message_to_one_session`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_8_2_a_shared_subscription_alternates_and_sends_no_retained_message`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_7_2_a_wildcard_filter_does_not_match_a_topic_beginning_with_dollar`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_2_2_a_topic_name_with_a_wildcard_gets_0x90`;
against Mosquitto
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_8_2_two_mosquitto_v5_clients_share_a_subscription`.

**Status:** done — the test against Mosquitto is `#[ignore]` (job
`integration-bridges-vendor`), green on the Linux test host.

### §4.9 Flow control — server

**Spec:** [MQTT-4.9.0-1] to [MQTT-4.9.0-3]; §3.3.4 the server's Receive
Maximum.

**Repo:** `crates/mqtt-bridge/src/broker.rs::Broker::{take_sendable,
resend}` (send quota = the client's Receive Maximum minus the deliveries
unacknowledged on this connection; it starts again with each connection,
also for the retransmission at reconnect; QoS 0 and every other packet
keep flowing);
`crates/mqtt-bridge/src/server/conn.rs::Connection::on_publish` (more
unacknowledged QoS 2 messages than the server's Receive Maximum: 0x93).

**Tests:** `broker::tests::mqtt5_4_9_the_send_quota_holds_back_qos_1_and_2_not_qos_0`,
`broker::tests::mqtt5_4_9_a_reconnect_resends_within_the_new_receive_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_9_the_server_sends_no_more_than_the_clients_receive_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_9_a_reconnect_resends_no_more_than_the_new_receive_maximum`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_3_3_4_a_client_over_the_servers_receive_maximum_gets_0x93`.

**Status:** done

### §4.11-§4.12 Server redirection and enhanced authentication — server

**Spec:** §4.11; [MQTT-4.12.0-1] to [MQTT-4.12.0-7], [MQTT-4.12.1-1],
[MQTT-4.12.1-2]; simple authentication with User Name and Password
(§5.4.1).

**Repo:** `crates/mqtt-bridge/src/server/mod.rs::{Authenticator,
AuthExchange, AuthStep, Redirect, ServerHandle::redirect_clients}`;
`crates/mqtt-bridge/src/server/conn.rs::Connection::{open, on_auth}` —
AUTH 0x18 until success, CONNACK and AUTH with the Authentication Method
of the CONNECT, an unknown method with 0x8C, re-authentication with 0x19,
a failure with DISCONNECT and the close, AUTH without an Authentication
Method in the CONNECT as a Protocol Error; redirection with CONNACK or
DISCONNECT 0x9C/0x9D and the Server Reference.

**Tests:** `crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_12_enhanced_authentication_and_re_authentication`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_12_simple_authentication_refuses_a_wrong_password`,
`crates/mqtt-bridge/tests/broker_server_conformance.rs::mqtt5_4_11_server_redirection_with_a_server_reference`.

**Status:** done — the methods themselves are implementations of
`Authenticator`; the test checks the exchange with a challenge-response
method of its own.

### §4.13 Handling errors — server

**Spec:** [MQTT-4.13.1-1], [MQTT-4.13.2-1]; [MQTT-1.5.4-1],
[MQTT-1.5.4-2] (UTF-8, U+0000); §3.1.2.11.4 Packet too large.

**Repo:** `crates/mqtt-bridge/src/server/conn.rs::Connection::{run,
close}` — a Malformed Packet, a Protocol Error and every other
error with a Reason Code ends with a DISCONNECT with that code (3.1.1:
only the close), then the Will and the session by §3.1.2.5; among them
the Protocol Errors of §3.2.2.3.11 to §3.2.2.3.13 (0xA2, 0x9E, 0xA1) and
a Topic Alias of 0 with 0x94;
`crates/mqtt-bridge/src/net.rs::read_packet_within` (the size limit before
the body is read, a Remaining Length over four octets);
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

### §5 Security considerations

**Spec:** §5 — security notes (TLS, authentication, authorization,
privacy); informative, no wire mapping.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — chapter 5 is marked "(non-normative)" in the spec.

## §6 WebSocket support

### §6 WebSocket transport for MQTT

**Spec:** §6 — "MQTT Control Packets MUST be sent in WebSocket binary data frames. If any other type of data frame is received the recipient MUST close the Network Connection [MQTT-6.0.0-1]." "The receiver MUST NOT assume that MQTT Control Packets are aligned on WebSocket frame boundaries [MQTT-6.0.0-2]." "The Client MUST include mqtt in the list of WebSocket Sub Protocols it offers [MQTT-6.0.0-3]." "The WebSocket Subprotocol name selected and returned by the Server MUST be mqtt [MQTT-6.0.0-4]."

**Repo:** `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`WsStream`: offers the subprotocol `mqtt` and refuses an answer without or with another subprotocol; sends each packet in one binary frame; reads the payloads of binary and continuation frames as one byte stream; a text frame is followed by a Close frame with 1003 and the connection closed), `crates/mqtt-bridge/src/daemon/client.rs` (`MqttClient::connect_url`, `ws://`, `wss://`)

**Tests:** `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_1_a_text_frame_from_the_server_closes_the_network_connection`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_2_a_control_packet_split_over_frames_is_read_as_one`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_2_two_control_packets_in_one_frame_are_both_read`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_3_the_client_offers_the_subprotocol_mqtt`, `crates/mqtt-bridge/tests/websocket_client_rfc6455.rs::mqtt5_6_0_0_4_a_server_that_selects_no_or_another_subprotocol_is_refused`; against Mosquitto `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_ws_broker_url_carries_mqtt_both_ways_through_mosquitto`, `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate`, `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_a_ws_url_on_a_plain_listener_fails_the_handshake`

**Status:** done — the client side holds [MQTT-6.0.0-1] to [MQTT-6.0.0-4]; the five tests with the server in the same process run in the job `merge-rust-mqtt-bridge`, the three tests against Mosquitto are `#[ignore]` and run in the weekly job `integration-bridges-vendor`. The server side is the next item.

### §6 MQTT server over WebSocket

**Spec:** §6 — [MQTT-6.0.0-1], [MQTT-6.0.0-2] and [MQTT-6.0.0-4] on the server side: binary frames, packets across frame boundaries, select the subprotocol `mqtt`.

**Repo:** `crates/mqtt-bridge/src/server/mod.rs::{MqttBrokerServer::with_websocket, MqttBrokerServer::with_secure_websocket, MqttBrokerServer::with_tls, Transport, ServerHandle::transport_addr, ServerHandle::handshake_failures}` — listeners for WebSocket (`ws`), WebSocket over TLS (`wss`) and TLS (`mqtts`) beside the TCP listener (features `websocket` and `tls`, both in `daemon`); the handshakes run on the connection's thread. `crates/mqtt-bridge/src/server/websocket.rs::{accept, WebSocketStream}` — the opening handshake of RFC 6455 §4.2 selects the subprotocol `mqtt`; a request without `mqtt` gets HTTP 400, an invalid one the status of its fault (426, 405, 400), both the close; the path of the request has no effect on MQTT, every path reaches the same server. The payloads of binary and continuation frames are one byte stream, read as they arrive; no frame is buffered whole, the limit is the Maximum Packet Size of MQTT. A text frame closes the Network Connection with a Close frame 1003, recognised from its header. An unmasked frame, an RSV bit, a reserved opcode, a fragmentation error and an invalid Close frame fail the connection with 1002 or 1007. Each packet of the server goes out in one unmasked binary frame; a Ping gets a Pong, a Close a Close with the same status code. When the server closes, Close 1000 and the end of its direction follow; the TCP connection closes when the client's Close frame has come, otherwise after two seconds (`crates/mqtt-bridge/src/server/websocket.rs::CLOSE_WAIT`). `crates/mqtt-bridge/src/server/tls.rs::TlsStream` — TLS with the handshake of the bridges and the caller's `rustls::ServerConfig`. `crates/mqtt-bridge/src/ws.rs` — the subprotocol, the reading of the handshake head and the Close reason, shared with the client.

**Tests:** `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_4_the_server_selects_the_subprotocol_mqtt`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_4_a_client_that_does_not_offer_mqtt_is_refused`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_request_path_has_no_impact_on_mqtt`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_an_invalid_opening_handshake_gets_an_http_error_response`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_opening_handshake_has_a_deadline`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_1_a_text_frame_closes_the_network_connection`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_1_the_server_sends_each_control_packet_in_one_unmasked_binary_frame`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_2_a_control_packet_split_over_frames_is_read_as_one`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_0_0_2_several_control_packets_in_one_frame_are_all_handled`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_invalid_frames_of_the_client_fail_the_connection`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_a_close_frame_of_the_client_is_answered_and_its_will_published`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_server_ends_a_connection_with_disconnect_then_a_close_frame`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_without_a_close_frame_of_the_client_the_server_closes_after_close_wait`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_server_answers_a_ping_with_a_pong_of_the_same_data`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_a_message_crosses_between_the_tcp_and_the_websocket_listener`, `crates/mqtt-bridge/tests/websocket_server.rs::mqtt5_6_the_zerodds_client_publishes_and_subscribes_over_wss_and_mqtts`, `crates/mqtt-bridge/tests/contract_websocket_bridge.rs::ws_4_2_1_a_request_is_read_with_its_offered_subprotocols_and_refused_with_its_status`, `crates/mqtt-bridge/tests/contract_websocket_bridge.rs::ws_4_2_2_the_response_carries_the_accept_value_and_only_the_selected_subprotocol`, `crates/mqtt-bridge/tests/contract_websocket_bridge.rs::ws_5_2_the_header_of_a_frame_is_read_before_its_payload`, `crates/mqtt-bridge/tests/contract_websocket_bridge.rs::ws_7_4_1_the_status_codes_and_the_close_body`, `crates/mqtt-bridge/tests/contract_bridge_security.rs::mqtt5_4_2_serve_tls_handshake_finishes_the_handshake_or_fails_within_its_timeout`; against Eclipse Paho (the Python client `paho-mqtt`, `transport="websockets"`) `crates/mqtt-bridge/tests/paho_websocket_server.rs::mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_websocket`, `crates/mqtt-bridge/tests/paho_websocket_server.rs::mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_wss`.

**Status:** done — the 16 tests in `websocket_server.rs`, the four tests of the contract with `zerodds-websocket-bridge` and the test of the contract with `zerodds-bridge-security` run in the job `merge-rust-mqtt-bridge`; the two tests against Paho are `#[ignore]` and run in the weekly job `integration-bridges-vendor` (entry `paho` in `tests/integration/provision.sh`). Green on the Linux test host, against Paho 2.1.0 (Debian package `python3-paho-mqtt`) in MQTT 5.0 and 3.1.1 over `ws` and `wss`.

## §7 Conformance

### §7 Conformance targets

**Spec:** §7 — three conformance targets: MQTT client, MQTT server, MQTT
server with MQTT bridge functionality.

**Repo:** wire codec + broker logic + network server in the `mqtt-bridge`
crate. The targets *MQTT client* (`net.rs::MqttClient`) and *MQTT server*
(`crates/mqtt-bridge/src/server/mod.rs::MqttBrokerServer`) are realized and interop-tested against the
Eclipse Mosquitto reference (`tests/mosquitto_interop_e2e.rs`), the server over WebSocket also
against Eclipse Paho (`tests/paho_websocket_server.rs`).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs`, `crates/mqtt-bridge/tests/paho_websocket_server.rs::mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_websocket`, `crates/mqtt-bridge/tests/paho_websocket_server.rs::mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_wss`.

**Status:** n/a (informative) — names the conformance targets client, server and server with bridge functionality; the requirements are the items of this document.

---

## Audit status

37 done / 0 partial / 0 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document mqtt-5.0.md`):

* `cargo nextest run --profile merge -p zerodds-mqtt-bridge --all-targets` (job `merge-rust-mqtt-bridge`) — 45 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor` (job `integration-bridges-vendor`) — 7 tests green.

52 cited tests green, 0 not green.

