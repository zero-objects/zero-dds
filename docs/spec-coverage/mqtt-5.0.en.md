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

**Tests:** `data_types::tests::two_byte_int_round_trip`,
`utf8_string_round_trip`, `utf8_string_starts_with_be_length`,
`utf8_string_decode_invalid_utf8`, `utf8_string_decode_truncated`,
`binary_data_round_trip`; `vbi::tests::*` (10 tests, all boundaries
0/127/128/16383/16384/2097151/2097152/MAX, 5-byte reject).

**Status:** done

---

## §2 MQTT control packet format

### §2.1 Structure of an MQTT control packet

**Spec:** §2.1, p. 19-21 — Fixed Header (type nibble + flags nibble +
Remaining Length VBI), Variable Header, Payload.

**Repo:** `crates/mqtt-bridge/src/packet.rs::FixedHeader`.

**Tests:** `packet::tests::publish_header_decodes_dup_qos_retain_flags`,
`codec::tests::fixed_header_first_byte_layout_for_publish`.

**Status:** done

### §2.1.2 Control packet type (Table 2-1)

**Spec:** §2.1.2 — 15 packet types (CONNECT=1, ..., AUTH=15).

**Repo:** `crates/mqtt-bridge/src/packet.rs::ControlPacketType` with all 15
values.

**Tests:** `packet::tests::packet_type_round_trip_via_bits`,
`packet_type_zero_is_reserved`.

**Status:** done

### §2.1.4 Remaining Length

**Spec:** §2.1.4 — VBI; bytes after the Fixed Header to the end of the
packet.

**Repo:** cross-ref the VBI module.

**Tests:** `codec::tests::large_payload_encodes_multibyte_remaining_length`,
`truncated_remaining_length_decode_fails`.

**Status:** done

### §2.2 Variable Header

**Spec:** §2.2, p. 21 — per-packet-type variable-header content.

**Repo:** the PUBLISH variable header in
`crates/mqtt-bridge/src/codec.rs::encode_publish/decode_publish`.

**Tests:** cross-ref `publish_qos*_round_trip`.

**Status:** done — for PUBLISH; further packet types implementable
analogously.

### §2.2.2 Properties

**Spec:** §2.2.2, p. 23-25 — a VBI-length-prefixed property block; property
identifier (spec Table 2-4) + value.

**Repo:** `crates/mqtt-bridge/src/properties.rs::{PropertyId, Property}`
with 15 named IDs + an `Other(u32)` variant. The codec serializes the
property-block body as opaque bytes with a VBI length prefix.

**Tests:** `properties::tests::property_id_round_trip_for_all_named`,
`property_id_unknown_yields_other_variant`,
`well_known_property_ids_match_spec_values`,
`codec::tests::empty_properties_round_trips_as_empty_vec`,
`non_empty_properties_round_trip_preserves_bytes`.

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

**Tests:** `control_packets::tests::connect_round_trip`.

**Status:** done

### §3.2 CONNACK

**Spec:** §3.2 — Connect Acknowledgment.

**Repo:** `control_packets::{encode_connack_body, decode_connack_body}` (`ConnackBody`).

**Tests:** `control_packets::tests::connack_round_trip`.

**Status:** done

### §3.3 PUBLISH — fully covered

**Spec:** §3.3, p. 48-58 — the complete PUBLISH packet spec with
DUP/QoS/RETAIN flags, topic name, packet identifier (at QoS>0), properties,
payload.

**Repo:** `crates/mqtt-bridge/src/codec.rs::encode_publish/decode_publish`
fully implemented; QoS-3 is rejected (MalformedPacket), the
packet-identifier requirement at QoS>0 is enforced.

**Tests:** `codec::tests::publish_qos0_no_packet_id_round_trip`,
`publish_qos1_includes_packet_id_round_trip`, `publish_qos2_round_trip`,
`invalid_qos_3_rejected_on_encode`, `missing_packet_id_at_qos1_rejected`,
`wrong_packet_type_rejected_on_decode`,
`fixed_header_first_byte_layout_for_publish`,
`empty_properties_round_trips_as_empty_vec`,
`non_empty_properties_round_trip_preserves_bytes`,
`truncated_remaining_length_decode_fails`, `empty_input_decode_fails`,
`large_payload_encodes_multibyte_remaining_length`.

**Status:** done

### §3.4 PUBACK

**Spec:** §3.4 — acknowledgment for a QoS-1 PUBLISH.

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}` (shared
`AckBody` structure).

**Tests:** `control_packets::tests::ack_round_trip`.

**Status:** done

### §3.5 PUBREC

**Spec:** §3.5 — QoS-2 receive acknowledgment (step 1 of the 4-way
handshake).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}` (same
AckBody structure).

**Tests:** `control_packets::tests::ack_round_trip`.

**Status:** done

### §3.6 PUBREL

**Spec:** §3.6 — QoS-2 publish release (step 2).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`.

**Tests:** `control_packets::tests::ack_round_trip`.

**Status:** done

### §3.7 PUBCOMP

**Spec:** §3.7 — QoS-2 publish complete (step 3, the final ACK).

**Repo:** `control_packets::{encode_ack_body, decode_ack_body}`.

**Tests:** `control_packets::tests::ack_round_trip`.

**Status:** done

### §3.8 SUBSCRIBE

**Spec:** §3.8 — client-to-server subscription request with a topic-filter
list.

**Repo:** `control_packets::{encode_subscribe_body, decode_subscribe_body}`.

**Tests:** `control_packets::tests::subscribe_round_trip`.

**Status:** done

### §3.9 SUBACK

**Spec:** §3.9 — server-to-client SUBSCRIBE acknowledgment with reason codes
per subscription.

**Repo:** `control_packets::{encode_suback_body, decode_suback_body}`.

**Tests:** `control_packets::tests::suback_round_trip`.

**Status:** done

### §3.10 UNSUBSCRIBE

**Spec:** §3.10 — client unsubscribe request.

**Repo:** `control_packets::{encode_unsubscribe_body, decode_unsubscribe_body}`.

**Tests:** `control_packets::tests::unsubscribe_round_trip`.

**Status:** done

### §3.11 UNSUBACK

**Spec:** §3.11 — server UNSUBSCRIBE acknowledgment with reason codes.

**Repo:** `control_packets::{encode_unsuback_body, decode_unsuback_body}`.

**Tests:** `control_packets::tests::unsuback_round_trip`.

**Status:** done

### §3.12 PINGREQ

**Spec:** §3.12 — a 2-byte heartbeat (type byte + Remaining=0).

**Repo:** constant `0xC0`; encode trivial (1 byte 0xC0 + VBI(0)).

**Tests:** `packet::tests::packet_type_round_trip_via_bits`.

**Status:** done

### §3.13 PINGRESP

**Spec:** §3.13 — a 2-byte heartbeat reply (type byte + Remaining=0).

**Repo:** constant `0xD0`; encode trivial (1 byte 0xD0 + VBI(0)).

**Tests:** `packet::tests::packet_type_round_trip_via_bits`.

**Status:** done

### §3.14 DISCONNECT

**Spec:** §3.14 — connection teardown (with a reason code, new in 5.0).

**Repo:** `control_packets::{encode_disconnect_body, decode_disconnect_body}`.

**Tests:** `control_packets::tests::disconnect_round_trip`.

**Status:** done

### §3.15 AUTH

**Spec:** §3.15 — extended authentication (new in 5.0, multi-step
SASL-like challenge/response).

**Repo:** `control_packets::{encode_auth_body, decode_auth_body}`.

**Tests:** `control_packets::tests::auth_round_trip`.

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

**Tests:** inline (~99 inline `#[test]` across mqtt-bridge incl.
`keep_alive::tests::*` (5 tests)).

**Status:** done — session manager, subscription tree, retained messages,
will handling, QoS levels (0/1/2) + the keep-alive timer implemented
spec-conformantly.

### §4 Standalone broker server (network exposure of the broker logic)

**Spec:** §4 + §7 (conformance target *MQTT server*) — a listening broker
accepts client connections, handles CONNECT/CONNACK, fans PUBLISH out to
subscribers, and runs the QoS 0/1/2 protocol flow.

**Repo:** `crates/mqtt-bridge/src/server.rs::{MqttBrokerServer, ServerHandle}`
— a `TcpListener` + accept loop, per connection a reader/writer thread
(event-driven via an mpsc channel), wiring the `broker.rs` engine:
CONNECT/CONNACK, SUBSCRIBE/SUBACK (+ retained replay §3.3.1.3), PUBLISH
fan-out, QoS 0/1/2 (exactly-once delivers on PUBREL, §4.3.3),
UNSUBSCRIBE/UNSUBACK, PINGREQ/PINGRESP, will on abnormal disconnect
(§3.1.2.5). The protocol version is negotiated per client from the CONNECT
protocol level — 5.0 and 3.1.1 at the same broker.
`crates/mqtt-bridge/src/net.rs::{MqttClient, read_packet}` — TCP framing + a
lightweight version-aware client.

**Tests:** `crates/mqtt-bridge/tests/broker_server_e2e.rs` (6, in-process:
pub/sub, retained, wildcards, QoS 2, cross-version 3.1.1⇄5.0); live
`crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs` (real
`mosquitto_pub`/`mosquitto_sub` 5.0 + 3.1.1 against the ZeroDDS broker).

**Status:** done

---

## §5 Security

### §5 Security considerations

**Spec:** §5 — security notes (TLS, authentication, authorization,
privacy); informative, no wire mapping.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the security considerations are
informative; concrete TLS bindings are caller-layer.

## §6 WebSocket support

### §6 WebSocket transport for MQTT

**Spec:** §6 — MQTT 5.0 over WebSocket (RFC 6455, sub-protocol `mqtt`).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — the WebSocket layer is its own crate
(`crates/websocket-bridge/`, see `websocket-rfc-6455.md`); MQTT-over-WebSocket
is a caller-layer composition (passing the `crates/mqtt-bridge` frame codec
through WebSocket frames).

## §7 Conformance

### §7 Conformance targets

**Spec:** §7 — three conformance targets: MQTT client, MQTT server, MQTT
server with MQTT bridge functionality.

**Repo:** wire codec + broker logic + network server in the `mqtt-bridge`
crate. The targets *MQTT client* (`net.rs::MqttClient`) and *MQTT server*
(`server.rs::MqttBrokerServer`) are realized and interop-tested against the
Eclipse Mosquitto reference (`tests/mosquitto_interop_e2e.rs`).

**Tests:** `crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs`.

**Status:** `n/a (informative)` — the conformance statement is declarative;
ZeroDDS realizes the client and server targets but declares no formal
conformance marker at the crate.

---

## Audit status

23 done / 0 partial / 0 open / 4 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-mqtt-bridge` — 165 lib inline tests + 24
integration tests green, 0 failed (modules: `broker`, `codec`,
`control_packets`, `data_types`, `dds_bridge`, `net`, `packet`, `properties`,
`reason_codes`, `server`, `topic_filter`, `vbi`, `version`). Live interop
additionally (codepit): `cargo test -p zerodds-mqtt-bridge --test
mosquitto_interop_e2e -- --ignored` — 5 tests green against Eclipse
Mosquitto 2.0.

No open items.
