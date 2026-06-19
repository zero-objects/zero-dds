# OASIS MQTT v3.1.1 — Spec Coverage

**Spec:** [OASIS MQTT Version 3.1.1 — OASIS Standard, 29 October 2014 →](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html)

**Context:** MQTT 3.1.1 (protocol level **4**) shares the control-packet framing
(§2.1) with MQTT 5.0 but differs in the variable header / payload: **no property
blocks** (introduced in 5.0), a 2-byte CONNACK, a UNSUBACK without reason codes,
an empty DISCONNECT body, and no AUTH packet. ZeroDDS implements 3.1.1 as a
version-aware path in the same `crates/mqtt-bridge` codec
(`ProtocolVersion::V311`); the broker (`server.rs`) and client (`net.rs`)
negotiate the version per connection from the CONNECT protocol level. Every item
is backed by a repo path + a test; interop is proven live against Eclipse
Mosquitto 2.0 (`-V mqttv311`), both directions and both roles (client against a
Mosquitto broker, real Mosquitto clients against the ZeroDDS broker).

Implementation:

- `crates/mqtt-bridge/` — version-aware 3.1.1 codec (`ProtocolVersion::V311`)
  + broker (`server.rs`) + client (`net.rs`); 165 lib tests + 6 broker E2E +
  5 Mosquitto-live green.

---

## §3.1.2.2 Protocol level + version negotiation

**Spec:** §3.1.2.1-§3.1.2.2 — protocol name `"MQTT"` + protocol level `4`. The
server accepts/rejects based on the level.

**Repo:** `crates/mqtt-bridge/src/version.rs::ProtocolVersion` (`V311`/`V5`,
`level`/`from_level`/`has_properties`/`protocol_name`). The broker reads the
level from the CONNECT body and replies in the matching dialect
(`server.rs::handle_connection`).

**Tests:** `version::tests::{level_round_trip, unsupported_levels_rejected,
only_v5_has_properties}`.

**Status:** done

## §2.1 Frame format (shared with 5.0)

**Spec:** §2 — fixed header (`type` nibble + `flags` nibble + Remaining Length
VBI). Identical to MQTT 5.0.

**Repo:** `crates/mqtt-bridge/src/packet.rs::{FixedHeader, ControlPacketType}` +
`vbi.rs` (Remaining-Length VBI) + `net.rs::{read_packet, frame_packet}`.

**Tests:** `packet::tests::*`, `vbi::tests::*`, `net::tests::{
frame_and_read_round_trip, read_packet_handles_multibyte_remaining_length}`.

**Status:** done

## §3.1 CONNECT

**Spec:** §3.1 v3.1.1 — like 5.0 but **without** the CONNECT and Will property
blocks.

**Repo:** `control_packets::{encode_connect_body_v, decode_connect_body_v}` with
`ProtocolVersion::V311` (drops both property blocks).

**Tests:** `control_packets::tests::v311_connect_omits_property_blocks`; live
`mosquitto_interop_e2e.rs` (3.1.1 handshake against Mosquitto + ZeroDDS broker).

**Status:** done

## §3.2 CONNACK

**Spec:** §3.2 v3.1.1 — exactly 2 bytes (acknowledge flags + return code), no
property block.

**Repo:** `control_packets::{encode_connack_body_v, decode_connack_body_v}`.

**Tests:** `control_packets::tests::v311_connack_is_exactly_two_bytes`.

**Status:** done

## §3.3 PUBLISH

**Spec:** §3.3 v3.1.1 — topic + (at QoS>0) packet identifier + payload, **without**
a property block.

**Repo:** `codec::{encode_publish_v, decode_publish_v}` with `V311`.

**Tests:** `codec::tests::{v311_publish_has_no_property_block,
v311_publish_qos0_round_trip}`; live `mosquitto_interop_e2e.rs`.

**Status:** done

## §3.4-§3.7 PUBACK / PUBREC / PUBREL / PUBCOMP

**Spec:** §3.4-§3.7 v3.1.1 — exactly the 2-byte packet identifier (no reason
code, no properties).

**Repo:** `control_packets::{encode_ack_body_v, decode_ack_body}` (`V311` emits
only the packet identifier; the shared decoder reads the short form). The QoS-2
flow in the broker: `server.rs` (PUBREC→PUBREL→PUBCOMP).

**Tests:** `control_packets::tests::v311_ack_is_packet_id_only`; in-process
`broker_server_e2e::qos2_exactly_once_delivery`.

**Status:** done

## §3.8 SUBSCRIBE

**Spec:** §3.8 v3.1.1 — packet identifier + (topic filter + QoS byte) list, no
property block.

**Repo:** `control_packets::{encode_subscribe_body_v, decode_subscribe_body_v}`.

**Tests:** `control_packets` round-trip + live `mosquitto_interop_e2e.rs`.

**Status:** done

## §3.9 SUBACK

**Spec:** §3.9 v3.1.1 — packet identifier + return codes (granted QoS 0/1/2 or
0x80 = failure), no property block.

**Repo:** `control_packets::{encode_suback_body_v, decode_suback_body_v}`.

**Tests:** `control_packets::tests::v311_suback_has_no_property_block`.

**Status:** done

## §3.10-§3.11 UNSUBSCRIBE / UNSUBACK

**Spec:** §3.10-§3.11 v3.1.1 — UNSUBSCRIBE = packet identifier + filter list (no
properties); **UNSUBACK = the packet identifier only** (no reason codes — those
arrived in 5.0).

**Repo:** `control_packets::{encode_unsubscribe_body_v,
decode_unsubscribe_body_v, encode_unsuback_body_v, decode_unsuback_body_v}`.

**Tests:** `control_packets::tests::v311_unsuback_is_packet_id_only_no_reason_codes`.

**Status:** done

## §3.12-§3.13 PINGREQ / PINGRESP

**Spec:** §3.12-§3.13 — body-less keep-alive packets. Identical to 5.0.

**Repo:** `server.rs` (PINGREQ → PINGRESP); wire constants in `packet.rs`.

**Tests:** covered by the broker e2e (keep-alive path).

**Status:** done

## §3.14 DISCONNECT

**Spec:** §3.14 v3.1.1 — an **empty** body (no reason code, no properties); the
fixed-header pair `0xE0 0x00` is the whole packet.

**Repo:** `control_packets::encode_disconnect_body_v` (`V311` → empty body) +
`net.rs::MqttClient::disconnect`.

**Tests:** `control_packets::tests::v311_disconnect_body_is_empty`.

**Status:** done

## §4 Operational behavior (3.1.1 at the broker + client)

**Spec:** §4 — session state, subscriptions, QoS flow, retained, will. Identical
semantics to 5.0; only the wire encoding of the replies is 3.1.1.

**Repo:** `server.rs::MqttBrokerServer` + `net.rs::MqttClient` (both
version-aware) over the shared `broker.rs` engine.

**Tests:** in-process `broker_server_e2e::{cross_version_v5_publishes_v311_subscribes,
cross_version_v311_publishes_v5_subscribes}`; live `mosquitto_interop_e2e::{
zerodds_311_client_receives_from_mosquitto, mosquitto_311_client_receives_from_zerodds,
mosquitto_v311_clients_through_zerodds_broker}`.

**Status:** done

## §3.15 AUTH

**Spec:** — MQTT 3.1.1 has **no** AUTH packet (introduced only in 5.0, §3.15
v5.0).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — not part of the 3.1.1 spec.

---

## Audit status

12 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-mqtt-bridge --lib` — 165 tests green (incl. the
`v311_*` codec tests + the `version` module); in-process
`cargo test -p zerodds-mqtt-bridge --test broker_server_e2e` — 6 green. Live
interop (codepit, Eclipse Mosquitto 2.0): `MQTT_MOSQUITTO=1 cargo test -p
zerodds-mqtt-bridge --test mosquitto_interop_e2e -- --ignored` — 5 green.
