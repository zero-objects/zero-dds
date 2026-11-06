# OASIS MQTT v3.1.1 — Spec-Coverage

**Spec:** [OASIS MQTT Version 3.1.1 — OASIS Standard, 29 October 2014 →](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html)

**Scope-Hinweis:** MQTT 3.1.1 (Protocol Level **4**) teilt das Control-Packet-
Framing (§2.1) mit MQTT 5.0, unterscheidet sich aber im Variable Header /
Payload: **keine Property-Blocks** (in 5.0 eingeführt), 2-Byte-CONNACK,
UNSUBACK ohne Reason-Codes, leerer DISCONNECT-Body, kein AUTH-Packet. ZeroDDS
implementiert 3.1.1 als version-aware Pfad im selben `crates/mqtt-bridge`-Codec
(`ProtocolVersion::V311`); der Broker (`crates/mqtt-bridge/src/server.rs`) und Client (`net.rs`)
handeln die Version pro Verbindung aus dem CONNECT-Protocol-Level aus. Jedes
Item ist mit Repo + Test belegt; Interop ist live gegen Eclipse Mosquitto 2.0
(`-V mqttv311`) bewiesen, in beide Richtungen und in beiden Rollen
(Client gegen Mosquitto-Broker, echte Mosquitto-Clients gegen ZeroDDS-Broker).

Implementation:

- `crates/mqtt-bridge/` — version-aware 3.1.1-Codec (`ProtocolVersion::V311`)
  + Broker (`crates/mqtt-bridge/src/server.rs`) + Client (`net.rs`); 165 lib-Tests + 6 Broker-E2E +
  5 Mosquitto-Live grün.

---

## §3.1.2.2 Protocol Level + Versions-Negotiation

**Spec:** §3.1.2.1-§3.1.2.2 — Protocol Name `"MQTT"` + Protocol Level `4`. Der
Server akzeptiert/verweigert anhand des Levels.

**Repo:** `crates/mqtt-bridge/src/version.rs::ProtocolVersion` (`V311`/`V5`,
`level`/`from_level`/`has_properties`/`protocol_name`). Der Broker liest das
Level aus dem CONNECT-Body und antwortet im passenden Dialekt
(`crates/mqtt-bridge/src/server.rs::handle_connection`).

**Tests:** `version::tests::{mqtt311_3_1_2_2_level_round_trip, mqtt311_3_1_2_2_unsupported_levels_rejected,
mqtt311_3_1_2_2_only_v5_has_properties}`.

**Status:** done

## §2.1 Frame-Format (geteilt mit 5.0)

**Spec:** §2 — Fixed Header (`type`-Nibble + `flags`-Nibble + Remaining-Length-
VBI). Identisch zu MQTT 5.0.

**Repo:** `crates/mqtt-bridge/src/packet.rs::{FixedHeader, ControlPacketType}`
+ `vbi.rs` (Remaining-Length-VBI) + `net.rs::{read_packet, frame_packet}`.

**Tests:** `packet::tests::*`, `net::tests::{
mqtt311_2_2_a_frame_is_type_and_flags_remaining_length_and_body, mqtt311_2_1_read_packet_handles_multibyte_remaining_length}`.

**Status:** done

## §3.1 CONNECT

**Spec:** §3.1 v3.1.1 — wie 5.0, aber **ohne** CONNECT- und Will-Property-Blocks.

**Repo:** `control_packets::{encode_connect_body_v, decode_connect_body_v}` mit
`ProtocolVersion::V311` (lässt beide Property-Blocks weg).

**Tests:** `control_packets::tests::mqtt311_3_1_v311_connect_omits_property_blocks`; Live
`mosquitto_interop_e2e.rs` (3.1.1-Handshake gegen Mosquitto + ZeroDDS-Broker).

**Status:** done

## §3.2 CONNACK

**Spec:** §3.2 v3.1.1 — genau 2 Byte (Acknowledge Flags + Return Code), kein
Property-Block.

**Repo:** `control_packets::{encode_connack_body_v, decode_connack_body_v}`.

**Tests:** `control_packets::tests::mqtt311_3_2_v311_connack_is_exactly_two_bytes`.

**Status:** done

## §3.3 PUBLISH

**Spec:** §3.3 v3.1.1 — Topic + (bei QoS>0) Packet-Identifier + Payload, **ohne**
Property-Block.

**Repo:** `codec::{encode_publish_v, decode_publish_v}` mit `V311`.

**Tests:** `codec::tests::{mqtt311_3_3_2_the_publish_of_figure_3_11_has_no_property_block,
mqtt311_3_3_2_the_publish_of_figure_3_11_has_no_property_block}`; Live `mosquitto_interop_e2e.rs`.

**Status:** done

## §3.4-§3.7 PUBACK / PUBREC / PUBREL / PUBCOMP

**Spec:** §3.4-§3.7 v3.1.1 — genau der 2-Byte Packet-Identifier (kein Reason-
Code, keine Properties).

**Repo:** `control_packets::{encode_ack_body_v, decode_ack_body}` (`V311`
emittiert nur den Packet-Identifier; der gemeinsame Decoder liest die
Short-Form). QoS-2-Flow im Broker: `crates/mqtt-bridge/src/server.rs` (PUBREC→PUBREL→PUBCOMP).

**Tests:** `control_packets::tests::mqtt311_3_4_v311_ack_is_packet_id_only`; In-Process
`broker_server_e2e::mqtt5_4_qos2_exactly_once_delivery`.

**Status:** done

## §3.8 SUBSCRIBE

**Spec:** §3.8 v3.1.1 — Packet-Identifier + (Topic-Filter + QoS-Byte)-Liste,
ohne Property-Block.

**Repo:** `control_packets::{encode_subscribe_body_v, decode_subscribe_body_v}`.

**Tests:** `crates/mqtt-bridge/src/control_packets.rs::tests::mqtt5_3_8_the_subscribe_of_figures_3_19_and_3_21`,
`::mqtt5_3_10_the_unsubscribe_payload_of_figure_3_30`;
`crates/mqtt-bridge/src/topic_filter.rs::tests::mqtt311_3_8_validate_simple_filter`,
`::mqtt311_3_8_validate_plus_in_middle`, `::mqtt311_3_8_validate_hash_at_end`,
`::mqtt311_3_8_validate_hash_in_middle_rejected`, `::mqtt311_3_8_validate_mixed_wildcard_rejected`;
live `crates/mqtt-bridge/tests/mosquitto_interop_e2e.rs::mqtt5_4_mosquitto_311_client_receives_from_zerodds`.

**Status:** done

## §3.9 SUBACK

**Spec:** §3.9 v3.1.1 — Packet-Identifier + Return-Codes (Granted QoS 0/1/2 oder
0x80 = Failure), ohne Property-Block.

**Repo:** `control_packets::{encode_suback_body_v, decode_suback_body_v}`.

**Tests:** `control_packets::tests::mqtt311_3_9_v311_suback_has_no_property_block`.

**Status:** done

## §3.10-§3.11 UNSUBSCRIBE / UNSUBACK

**Spec:** §3.10-§3.11 v3.1.1 — UNSUBSCRIBE = Packet-Identifier + Filter-Liste
(keine Properties); **UNSUBACK = nur der Packet-Identifier** (keine Reason-Codes
— die kamen erst in 5.0).

**Repo:** `control_packets::{encode_unsubscribe_body_v,
decode_unsubscribe_body_v, encode_unsuback_body_v, decode_unsuback_body_v}`.

**Tests:** `control_packets::tests::mqtt311_3_10_v311_unsuback_is_packet_id_only_no_reason_codes`.

**Status:** done

## §3.12-§3.13 PINGREQ / PINGRESP

**Spec:** §3.12-§3.13 — body-lose Keep-Alive-Packets. Identisch zu 5.0.

**Repo:** `crates/mqtt-bridge/src/server.rs` (PINGREQ → PINGRESP, Body-los), `crates/mqtt-bridge/src/net.rs::MqttClient::ping` (sendet PINGREQ und prüft die Antwort gegen §3.13), Wire-Konstanten in `crates/mqtt-bridge/src/packet.rs::ControlPacketType`, Keep-Alive-Regel in `crates/mqtt-bridge/src/keep_alive.rs::KeepAliveTracker`.

**Tests:** `crates/mqtt-bridge/tests/broker_server_e2e.rs::mqtt5_4_pingreq_is_answered_with_a_bodyless_pingresp_within_the_keep_alive` — vier PINGREQ je Protokoll-Version (3.1.1 und 5.0) bei angekündigtem Keep Alive von 1 s; jede Antwort muss ein PINGRESP mit leerem Body und Null-Flags sein und innerhalb der Periode zurückkommen. Die 1,5-fache Ablauf-Regel selbst: `crates/mqtt-bridge/src/keep_alive.rs::tests::mqtt5_4_1_small_keep_alive_expires_after_1_5x`, `crates/mqtt-bridge/src/keep_alive.rs::tests::mqtt5_4_1_record_packet_resets_timer`, `crates/mqtt-bridge/src/keep_alive.rs::tests::mqtt5_4_1_keep_alive_zero_is_disabled`.

**Status:** done

## §3.14 DISCONNECT

**Spec:** §3.14 v3.1.1 — **leerer** Body (kein Reason-Code, keine Properties);
das Fixed-Header-Paar `0xE0 0x00` ist das ganze Packet.

**Repo:** `control_packets::encode_disconnect_body_v` (`V311` → leerer Body) +
`net.rs::MqttClient::disconnect`.

**Tests:** `control_packets::tests::mqtt311_3_14_v311_disconnect_body_is_empty`.

**Status:** done

## §4 Operational Behavior (3.1.1 am Broker + Client)

**Spec:** §4 — Session-State, Subscriptions, QoS-Flow, Retained, Will. Identische
Semantik zu 5.0; nur das Wire-Encoding der Replies ist 3.1.1.

**Repo:** `crates/mqtt-bridge/src/server.rs::MqttBrokerServer` + `net.rs::MqttClient` (beide
version-aware) über der gemeinsamen `crates/mqtt-bridge/src/broker.rs`-Engine.

**Tests:** In-Process `broker_server_e2e::{mqtt5_4_cross_version_v5_publishes_v311_subscribes,
mqtt5_4_cross_version_v311_publishes_v5_subscribes}`; Live `mosquitto_interop_e2e::{
mqtt5_4_zerodds_311_client_receives_from_mosquitto, mqtt5_4_mosquitto_311_client_receives_from_zerodds,
mqtt5_4_mosquitto_v311_clients_through_zerodds_broker}`.

**Status:** done

## §3.15 AUTH

**Spec:** — In MQTT 3.1.1 existiert **kein** AUTH-Packet (erst in 5.0, §3.15
v5.0 eingeführt).

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — nicht Teil der 3.1.1-Spec.

---

## Audit-Status

12 done / 0 partial / 0 open / 0 blocked / 1 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document mqtt-3.1.1.md`):

* `cargo nextest run --profile merge -p zerodds-mqtt-bridge --all-targets` (Job `merge-rust-mqtt-bridge`) — 26 Tests grün.
* `python3 ci/run_integration.py integration-bridges-vendor` (Job `integration-bridges-vendor`) — 3 Tests grün.

29 zitierte Tests grün, 0 nicht grün.

