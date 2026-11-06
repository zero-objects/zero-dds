# OASIS MQTT v3.1.1 — Spec Coverage

**Spec:** [OASIS MQTT Version 3.1.1 — OASIS Standard, 29 October 2014 →](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html)

**Context:** MQTT 3.1.1 (protocol level **4**) shares the control-packet framing
(§2.1) with MQTT 5.0 but differs in the variable header / payload: **no property
blocks** (introduced in 5.0), a 2-byte CONNACK, a UNSUBACK without reason codes,
an empty DISCONNECT body, and no AUTH packet. ZeroDDS implements 3.1.1 as a
version-aware path in the same `crates/mqtt-bridge` codec
(`ProtocolVersion::V311`); the broker (`crates/mqtt-bridge/src/server.rs`) and client (`net.rs`)
negotiate the version per connection from the CONNECT protocol level. Every item
is backed by a repo path + a test; interop is proven live against Eclipse
Mosquitto 2.0 (`-V mqttv311`), both directions and both roles (client against a
Mosquitto broker, real Mosquitto clients against the ZeroDDS broker).

Implementation:

- `crates/mqtt-bridge/` — version-aware 3.1.1 codec (`ProtocolVersion::V311`)
  + broker (`crates/mqtt-bridge/src/server.rs`) + client (`net.rs`); 165 lib tests + 6 broker E2E +
  5 Mosquitto-live green.

---

## §3.1.2.2 Protocol level + version negotiation

**Spec:** §3.1.2.1-§3.1.2.2 — protocol name `"MQTT"` + protocol level `4`. The
server accepts/rejects based on the level.

**Repo:** `crates/mqtt-bridge/src/version.rs::ProtocolVersion` (`V311`/`V5`,
`level`/`from_level`/`has_properties`/`protocol_name`). The broker reads the
level from the CONNECT body and replies in the matching dialect
(`crates/mqtt-bridge/src/server.rs::handle_connection`).

**Tests:** `version::tests::{mqtt311_3_1_2_2_level_round_trip, mqtt311_3_1_2_2_unsupported_levels_rejected,
mqtt311_3_1_2_2_only_v5_has_properties}`.

**Status:** done

## §2.1 Frame format (shared with 5.0)

**Spec:** §2 — fixed header (`type` nibble + `flags` nibble + Remaining Length
VBI). Identical to MQTT 5.0.

**Repo:** `crates/mqtt-bridge/src/packet.rs::{FixedHeader, ControlPacketType}` +
`vbi.rs` (Remaining-Length VBI) + `net.rs::{read_packet, frame_packet}`.

**Tests:** `packet::tests::*`, `net::tests::{
mqtt311_2_2_a_frame_is_type_and_flags_remaining_length_and_body, mqtt311_2_1_read_packet_handles_multibyte_remaining_length}`.

**Status:** done

## §3.1 CONNECT

**Spec:** §3.1 v3.1.1 — like 5.0 but **without** the CONNECT and Will property
blocks.

**Repo:** `control_packets::{encode_connect_body_v, decode_connect_body_v}` with
`ProtocolVersion::V311` (drops both property blocks).

**Tests:** `control_packets::tests::mqtt311_3_1_v311_connect_omits_property_blocks`; live
`mosquitto_interop_e2e.rs` (3.1.1 handshake against Mosquitto + ZeroDDS broker).

**Status:** done

## §3.2 CONNACK

**Spec:** §3.2 v3.1.1 — exactly 2 bytes (acknowledge flags + return code), no
property block.

**Repo:** `control_packets::{encode_connack_body_v, decode_connack_body_v}`.

**Tests:** `control_packets::tests::mqtt311_3_2_v311_connack_is_exactly_two_bytes`.

**Status:** done

## §3.3 PUBLISH

**Spec:** §3.3 v3.1.1 — topic + (at QoS>0) packet identifier + payload, **without**
a property block.

**Repo:** `codec::{encode_publish_v, decode_publish_v}` with `V311`.

**Tests:** `codec::tests::{mqtt311_3_3_2_the_publish_of_figure_3_11_has_no_property_block,
mqtt311_3_3_2_the_publish_of_figure_3_11_has_no_property_block}`; live `mosquitto_interop_e2e.rs`.

**Status:** done

## §3.4-§3.7 PUBACK / PUBREC / PUBREL / PUBCOMP

**Spec:** §3.4-§3.7 v3.1.1 — exactly the 2-byte packet identifier (no reason
code, no properties).

**Repo:** `control_packets::{encode_ack_body_v, decode_ack_body}` (`V311` emits
only the packet identifier; the shared decoder reads the short form). The QoS-2
flow in the broker: `crates/mqtt-bridge/src/server.rs` (PUBREC→PUBREL→PUBCOMP).

**Tests:** `control_packets::tests::mqtt311_3_4_v311_ack_is_packet_id_only`; in-process
`broker_server_e2e::mqtt5_4_qos2_exactly_once_delivery`.

**Status:** done

## §3.8 SUBSCRIBE

**Spec:** §3.8 v3.1.1 — packet identifier + (topic filter + QoS byte) list, no
property block.

**Repo:** `control_packets::{encode_subscribe_body_v, decode_subscribe_body_v}`.

**Tests:** `crates/mqtt-bridge/src/control_packets.rs::tests::mqtt5_3_8_the_subscribe_of_figures_3_19_and_3_21`,
`::mqtt5_3_10_the_unsubscribe_payload_of_figure_3_30`;
`crates/mqtt-bridge/src/topic_filter.rs::tests::mqtt311_3_8_validate_simple_filter`,
`::mqtt311_3_8_validate_plus_in_middle`, `::mqtt311_3_8_validate_hash_at_end`,
`::mqtt311_3_8_validate_hash_in_middle_rejected`, `::mqtt311_3_8_validate_mixed_wildcard_rejected`;
live `crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_311_client_receives_from_zerodds`.

**Status:** done

## §3.9 SUBACK

**Spec:** §3.9 v3.1.1 — packet identifier + return codes (granted QoS 0/1/2 or
0x80 = failure), no property block.

**Repo:** `control_packets::{encode_suback_body_v, decode_suback_body_v}`.

**Tests:** `control_packets::tests::mqtt311_3_9_v311_suback_has_no_property_block`.

**Status:** done

## §3.10-§3.11 UNSUBSCRIBE / UNSUBACK

**Spec:** §3.10-§3.11 v3.1.1 — UNSUBSCRIBE = packet identifier + filter list (no
properties); **UNSUBACK = the packet identifier only** (no reason codes — those
arrived in 5.0).

**Repo:** `control_packets::{encode_unsubscribe_body_v,
decode_unsubscribe_body_v, encode_unsuback_body_v, decode_unsuback_body_v}`.

**Tests:** `control_packets::tests::mqtt311_3_10_v311_unsuback_is_packet_id_only_no_reason_codes`.

**Status:** done

## §3.12-§3.13 PINGREQ / PINGRESP

**Spec:** §3.12-§3.13 — body-less keep-alive packets. Identical to 5.0.

**Repo:** `crates/mqtt-bridge/src/server.rs` (PINGREQ → PINGRESP, bodyless), `crates/mqtt-bridge/src/net.rs::MqttClient::ping` (sends PINGREQ and checks the reply against §3.13), wire constants in `crates/mqtt-bridge/src/packet.rs::ControlPacketType`, the keep-alive rule in `crates/mqtt-bridge/src/keep_alive.rs::KeepAliveTracker`.

**Tests:** `crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_pingreq_is_answered_with_a_bodyless_pingresp_within_the_keep_alive` — four PINGREQs per protocol version (3.1.1 and 5.0) with an announced Keep Alive of 1 s; every reply has to be a PINGRESP with an empty body and zero flags, and has to arrive inside the period. The 1.5x expiry rule itself: `crates/mqtt-bridge/src/keep_alive.rs::tests::mqtt5_4_1_small_keep_alive_expires_after_1_5x`, `crates/mqtt-bridge/src/keep_alive.rs::tests::mqtt5_4_1_record_packet_resets_timer`, `crates/mqtt-bridge/src/keep_alive.rs::tests::mqtt5_4_1_keep_alive_zero_is_disabled`.

**Status:** done

## §3.14 DISCONNECT

**Spec:** §3.14 v3.1.1 — an **empty** body (no reason code, no properties); the
fixed-header pair `0xE0 0x00` is the whole packet.

**Repo:** `control_packets::encode_disconnect_body_v` (`V311` → empty body) +
`net.rs::MqttClient::disconnect`.

**Tests:** `control_packets::tests::mqtt311_3_14_v311_disconnect_body_is_empty`.

**Status:** done

## §4 Operational behavior (3.1.1 at the broker + client)

**Spec:** §4 — session state, subscriptions, QoS flow, retained, will. Identical
semantics to 5.0; only the wire encoding of the replies is 3.1.1.

**Repo:** `crates/mqtt-bridge/src/server.rs::MqttBrokerServer` + `net.rs::MqttClient` (both
version-aware) over the shared `crates/mqtt-bridge/src/broker.rs` engine.

**Tests:** in-process `broker_server_e2e::{mqtt5_4_cross_version_v5_publishes_v311_subscribes,
mqtt5_4_cross_version_v311_publishes_v5_subscribes}`; live `mosquitto_interop_e2e::{
mqtt5_4_zerodds_311_client_receives_from_mosquitto, mqtt5_4_mosquitto_311_client_receives_from_zerodds,
mqtt5_4_mosquitto_v311_clients_through_zerodds_broker}`.

**Status:** done

## §3.15 AUTH

**Spec:** — MQTT 3.1.1 has **no** AUTH packet (introduced only in 5.0, §3.15
v5.0).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — not part of the 3.1.1 spec.

---

## Audit status

12 done / 0 partial / 0 open / 0 blocked / 1 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document mqtt-3.1.1.md`):

* `cargo nextest run --profile merge -p zerodds-mqtt-bridge --all-targets` (job `merge-rust-mqtt-bridge`) — 26 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor` (job `integration-bridges-vendor`) — 3 tests green.

29 cited tests green, 0 not green.

