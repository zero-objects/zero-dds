# OASIS MQTT v3.1.1 — Spec-Coverage

**Spec:** [OASIS MQTT Version 3.1.1 — OASIS Standard, 29 October 2014 →](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html)

**Scope-Hinweis:** MQTT 3.1.1 (Protocol Level **4**) teilt das Control-Packet-
Framing (§2.1) mit MQTT 5.0, unterscheidet sich aber im Variable Header /
Payload: **keine Property-Blocks** (in 5.0 eingeführt), 2-Byte-CONNACK,
UNSUBACK ohne Reason-Codes, leerer DISCONNECT-Body, kein AUTH-Packet. ZeroDDS
implementiert 3.1.1 als version-aware Pfad im selben `crates/mqtt-bridge`-Codec
(`ProtocolVersion::V311`); der Broker (`server.rs`) und Client (`net.rs`)
handeln die Version pro Verbindung aus dem CONNECT-Protocol-Level aus. Jedes
Item ist mit Repo + Test belegt; Interop ist live gegen Eclipse Mosquitto 2.0
(`-V mqttv311`) bewiesen, in beide Richtungen und in beiden Rollen
(Client gegen Mosquitto-Broker, echte Mosquitto-Clients gegen ZeroDDS-Broker).

Implementation:

- `crates/mqtt-bridge/` — version-aware 3.1.1-Codec (`ProtocolVersion::V311`)
  + Broker (`server.rs`) + Client (`net.rs`); 165 lib-Tests + 6 Broker-E2E +
  5 Mosquitto-Live grün.

---

## §3.1.2.2 Protocol Level + Versions-Negotiation

**Spec:** §3.1.2.1-§3.1.2.2 — Protocol Name `"MQTT"` + Protocol Level `4`. Der
Server akzeptiert/verweigert anhand des Levels.

**Repo:** `crates/mqtt-bridge/src/version.rs::ProtocolVersion` (`V311`/`V5`,
`level`/`from_level`/`has_properties`/`protocol_name`). Der Broker liest das
Level aus dem CONNECT-Body und antwortet im passenden Dialekt
(`server.rs::handle_connection`).

**Tests:** `version::tests::{level_round_trip, unsupported_levels_rejected,
only_v5_has_properties}`.

**Status:** done

## §2.1 Frame-Format (geteilt mit 5.0)

**Spec:** §2 — Fixed Header (`type`-Nibble + `flags`-Nibble + Remaining-Length-
VBI). Identisch zu MQTT 5.0.

**Repo:** `crates/mqtt-bridge/src/packet.rs::{FixedHeader, ControlPacketType}`
+ `vbi.rs` (Remaining-Length-VBI) + `net.rs::{read_packet, frame_packet}`.

**Tests:** `packet::tests::*`, `vbi::tests::*`, `net::tests::{
frame_and_read_round_trip, read_packet_handles_multibyte_remaining_length}`.

**Status:** done

## §3.1 CONNECT

**Spec:** §3.1 v3.1.1 — wie 5.0, aber **ohne** CONNECT- und Will-Property-Blocks.

**Repo:** `control_packets::{encode_connect_body_v, decode_connect_body_v}` mit
`ProtocolVersion::V311` (lässt beide Property-Blocks weg).

**Tests:** `control_packets::tests::v311_connect_omits_property_blocks`; Live
`mosquitto_interop_e2e.rs` (3.1.1-Handshake gegen Mosquitto + ZeroDDS-Broker).

**Status:** done

## §3.2 CONNACK

**Spec:** §3.2 v3.1.1 — genau 2 Byte (Acknowledge Flags + Return Code), kein
Property-Block.

**Repo:** `control_packets::{encode_connack_body_v, decode_connack_body_v}`.

**Tests:** `control_packets::tests::v311_connack_is_exactly_two_bytes`.

**Status:** done

## §3.3 PUBLISH

**Spec:** §3.3 v3.1.1 — Topic + (bei QoS>0) Packet-Identifier + Payload, **ohne**
Property-Block.

**Repo:** `codec::{encode_publish_v, decode_publish_v}` mit `V311`.

**Tests:** `codec::tests::{v311_publish_has_no_property_block,
v311_publish_qos0_round_trip}`; Live `mosquitto_interop_e2e.rs`.

**Status:** done

## §3.4-§3.7 PUBACK / PUBREC / PUBREL / PUBCOMP

**Spec:** §3.4-§3.7 v3.1.1 — genau der 2-Byte Packet-Identifier (kein Reason-
Code, keine Properties).

**Repo:** `control_packets::{encode_ack_body_v, decode_ack_body}` (`V311`
emittiert nur den Packet-Identifier; der gemeinsame Decoder liest die
Short-Form). QoS-2-Flow im Broker: `server.rs` (PUBREC→PUBREL→PUBCOMP).

**Tests:** `control_packets::tests::v311_ack_is_packet_id_only`; In-Process
`broker_server_e2e::qos2_exactly_once_delivery`.

**Status:** done

## §3.8 SUBSCRIBE

**Spec:** §3.8 v3.1.1 — Packet-Identifier + (Topic-Filter + QoS-Byte)-Liste,
ohne Property-Block.

**Repo:** `control_packets::{encode_subscribe_body_v, decode_subscribe_body_v}`.

**Tests:** `control_packets`-Roundtrip + Live `mosquitto_interop_e2e.rs`.

**Status:** done

## §3.9 SUBACK

**Spec:** §3.9 v3.1.1 — Packet-Identifier + Return-Codes (Granted QoS 0/1/2 oder
0x80 = Failure), ohne Property-Block.

**Repo:** `control_packets::{encode_suback_body_v, decode_suback_body_v}`.

**Tests:** `control_packets::tests::v311_suback_has_no_property_block`.

**Status:** done

## §3.10-§3.11 UNSUBSCRIBE / UNSUBACK

**Spec:** §3.10-§3.11 v3.1.1 — UNSUBSCRIBE = Packet-Identifier + Filter-Liste
(keine Properties); **UNSUBACK = nur der Packet-Identifier** (keine Reason-Codes
— die kamen erst in 5.0).

**Repo:** `control_packets::{encode_unsubscribe_body_v,
decode_unsubscribe_body_v, encode_unsuback_body_v, decode_unsuback_body_v}`.

**Tests:** `control_packets::tests::v311_unsuback_is_packet_id_only_no_reason_codes`.

**Status:** done

## §3.12-§3.13 PINGREQ / PINGRESP

**Spec:** §3.12-§3.13 — body-lose Keep-Alive-Packets. Identisch zu 5.0.

**Repo:** `server.rs` (PINGREQ → PINGRESP); Wire-Konstanten in `packet.rs`.

**Tests:** abgedeckt durch die Broker-e2e (Keep-Alive-Pfad).

**Status:** done

## §3.14 DISCONNECT

**Spec:** §3.14 v3.1.1 — **leerer** Body (kein Reason-Code, keine Properties);
das Fixed-Header-Paar `0xE0 0x00` ist das ganze Packet.

**Repo:** `control_packets::encode_disconnect_body_v` (`V311` → leerer Body) +
`net.rs::MqttClient::disconnect`.

**Tests:** `control_packets::tests::v311_disconnect_body_is_empty`.

**Status:** done

## §4 Operational Behavior (3.1.1 am Broker + Client)

**Spec:** §4 — Session-State, Subscriptions, QoS-Flow, Retained, Will. Identische
Semantik zu 5.0; nur das Wire-Encoding der Replies ist 3.1.1.

**Repo:** `server.rs::MqttBrokerServer` + `net.rs::MqttClient` (beide
version-aware) über der gemeinsamen `broker.rs`-Engine.

**Tests:** In-Process `broker_server_e2e::{cross_version_v5_publishes_v311_subscribes,
cross_version_v311_publishes_v5_subscribes}`; Live `mosquitto_interop_e2e::{
zerodds_311_client_receives_from_mosquitto, mosquitto_311_client_receives_from_zerodds,
mosquitto_v311_clients_through_zerodds_broker}`.

**Status:** done

## §3.15 AUTH

**Spec:** — In MQTT 3.1.1 existiert **kein** AUTH-Packet (erst in 5.0, §3.15
v5.0 eingeführt).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — nicht Teil der 3.1.1-Spec.

---

## Audit-Status

12 done / 0 partial / 0 open / 1 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-mqtt-bridge --lib` — 165 Tests grün (inkl.
der `v311_*`-Codec-Tests + `version`-Modul); In-Process
`cargo test -p zerodds-mqtt-bridge --test broker_server_e2e` — 6 grün.
Live-Interop (Linux bench host, Eclipse Mosquitto 2.0): `MQTT_MOSQUITTO=1 cargo test
-p zerodds-mqtt-bridge --test mosquitto_interop_e2e -- --ignored` — 5 grün.
