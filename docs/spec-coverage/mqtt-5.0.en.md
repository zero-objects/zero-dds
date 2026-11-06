# OASIS MQTT v5.0 — Spec Coverage

**Spec:** [OASIS MQTT Version 5.0 — Standard, 07 March 2019 →](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html)

**Context:** MQTT 5.0 is the OASIS pub-sub spec for IoT/M2M. ZeroDDS
implements the **wire codec** (spec §1.5 Data Types, §2.1 Fixed Header,
§2.2 Variable Header + Properties, §3.3 PUBLISH) as a pure-Rust no_std+alloc
library. Broker logic (session state, topic-filter matching,
QoS retransmission) is caller-layer.

Implementation:

- `crates/mqtt-bridge/` — wire codec, 5 modules, 36 tests green.

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

**Repo:** `crates/mqtt-bridge/src/properties.rs::{PropertyId, Property}`
with 15 named IDs + an `Other(u32)` variant. The codec serializes the
property-block body as opaque bytes with a VBI length prefix.

**Tests:** `properties::tests::mqtt5_2_2_2_each_named_property_has_its_identifier_of_table_2_4`,
`mqtt5_2_2_2_property_id_unknown_yields_other_variant`,
`codec::tests::mqtt5_3_3_2_3_the_properties_follow_their_length`.

**Status:** done — identifier registry + wire length prefix + a
`PropertyValueKind` table per spec §2.2.2.2 Table 2-4 with decode helpers
(`decode_two_byte_int`, `decode_four_byte_int`, `decode_utf8_string`,
`decode_binary_data`) and `PropertyId::value_kind()` as the central
dispatch.

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

**Spec:** §4 — session state, topic-filter matching, the QoS protocol flow,
keep-alive timer, message ordering, etc.

**Repo:** `crates/mqtt-bridge/src/broker.rs::{Broker, Session,
Subscription, RetainedMessage, DeliveryEnvelope, QoS, Will}` with
`connect`/`disconnect`/`subscribe`/`retained_for`/packet-ID tracking.
Topic-filter matching in `crates/mqtt-bridge/src/topic_filter.rs`. The
keep-alive timer as a wall-clock tick in
`crates/mqtt-bridge/src/keep_alive.rs::KeepAliveTracker` with
`record_packet`/`is_expired`/`remaining` and the spec-§3.1.2.10-conformant
1.5× period expiry rule.

**Tests:** `keep_alive::tests::mqtt5_4_1_keep_alive_zero_is_disabled`,
`keep_alive::tests::mqtt5_4_1_fresh_tracker_not_expired`,
`keep_alive::tests::mqtt5_4_1_record_packet_resets_timer`,
`keep_alive::tests::mqtt5_4_1_small_keep_alive_expires_after_1_5x`,
`keep_alive::tests::mqtt5_4_1_remaining_decreases_over_time`;
`broker::tests::mqtt5_4_1_the_qos_levels_are_0_1_and_2`; `topic_filter::tests::*`.

**Status:** done — session manager, subscription tree, retained messages,
will handling, QoS levels (0/1/2) + the keep-alive timer implemented
spec-conformantly.

### §4 Standalone broker server (network exposure of the broker logic)

**Spec:** §4 + §7 (conformance target *MQTT server*) — a listening broker
accepts client connections, handles CONNECT/CONNACK, fans PUBLISH out to
subscribers, and runs the QoS 0/1/2 protocol flow.

**Repo:** `crates/mqtt-bridge/src/server.rs::{MqttBrokerServer, ServerHandle}`
— a `TcpListener` + accept loop, per connection a reader/writer thread
(event-driven via an mpsc channel), wiring the `crates/mqtt-bridge/src/broker.rs` engine:
CONNECT/CONNACK, SUBSCRIBE/SUBACK (+ retained replay §3.3.1.3), PUBLISH
fan-out, QoS 0/1/2 (exactly-once delivers on PUBREL, §4.3.3),
UNSUBSCRIBE/UNSUBACK, PINGREQ/PINGRESP, will on abnormal disconnect
(§3.1.2.5). The protocol version is negotiated per client from the CONNECT
protocol level — 5.0 and 3.1.1 at the same broker.
`crates/mqtt-bridge/src/net.rs::{MqttClient, read_packet}` — TCP framing + a
lightweight version-aware client.

**Tests:** `crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_publish_subscribe_qos1`,
`mqtt5_4_retained_message_delivered_to_late_subscriber`, `mqtt5_4_wildcard_plus_and_hash`,
`mqtt5_4_qos2_exactly_once_delivery`, `mqtt5_4_cross_version_v5_publishes_v311_subscribes`,
`mqtt5_4_cross_version_v311_publishes_v5_subscribes`,
`mqtt5_4_pingreq_is_answered_with_a_bodyless_pingresp_within_the_keep_alive`; live
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_v5_clients_through_zerodds_broker`,
`mqtt5_4_mosquitto_v311_clients_through_zerodds_broker`,
`mqtt5_4_zerodds_client_through_zerodds_broker_cross_version`,
`mqtt5_4_zerodds_311_client_receives_from_mosquitto`,
`mqtt5_4_mosquitto_311_client_receives_from_zerodds`.

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

**Repo:** `crates/mqtt-bridge/src/daemon/ws_transport.rs` (`WsStream`, subprotocol `mqtt`), `crates/mqtt-bridge/src/daemon/client.rs`

**Tests:** `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_ws_broker_url_carries_mqtt_both_ways_through_mosquitto`, `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate`, `crates/mqtt-bridge/tests/websocket_broker.rs::mqtt5_6_a_ws_url_on_a_plain_listener_fails_the_handshake`

**Status:** partial — the client side (`ws://`, `wss://` in the daemon) holds the four rules and is tested against Mosquitto (the tests are `#[ignore]`, they need Mosquitto). The server (`crates/mqtt-bridge/src/server.rs`) accepts no WebSocket connection; own item.

### §6 MQTT server over WebSocket

**Spec:** §6 — [MQTT-6.0.0-1], [MQTT-6.0.0-2] and [MQTT-6.0.0-4] on the server side: binary frames, packets across frame boundaries, select the subprotocol `mqtt`.

**Repo:** —

**Tests:** —

**Status:** open — `MqttBrokerServer` (`crates/mqtt-bridge/src/server.rs`) has only a TCP listener.

## §7 Conformance

### §7 Conformance targets

**Spec:** §7 — three conformance targets: MQTT client, MQTT server, MQTT
server with MQTT bridge functionality.

**Repo:** wire codec + broker logic + network server in the `mqtt-bridge`
crate. The targets *MQTT client* (`net.rs::MqttClient`) and *MQTT server*
(`crates/mqtt-bridge/src/server.rs::MqttBrokerServer`) are realized and interop-tested against the
Eclipse Mosquitto reference (`tests/mosquitto_interop_e2e.rs`).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs`.

**Status:** n/a (informative) — names the conformance targets client, server and server with bridge functionality; the requirements are the items of this document.

---

## Audit status

23 done / 1 partial / 1 open / 0 blocked / 3 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document mqtt-5.0.md`):

* `cargo nextest run --profile merge -p zerodds-mqtt-bridge --all-targets` (job `merge-rust-mqtt-bridge`) — 45 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor` (job `integration-bridges-vendor`) — 7 tests green.

52 cited tests green, 0 not green.

Open items: `mqtt-5.0.open.en.md`.
